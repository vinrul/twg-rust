// End-to-end realtime smoke test for the twg.ws.v1 protocol.
//
// Requires a running server with PostgreSQL seeded from
// baseline/fixtures/seed.sql. The script resets the database first.
//
//   BASE_URL=http://127.0.0.1:8012 DB_CONTAINER=twg-phase7-db \
//   node tools/ws-smoke.mjs

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const baseUrl = (process.env.BASE_URL ?? "http://127.0.0.1:8012").replace(/\/+$/, "");
const password = process.env.TWG_PASSWORD ?? "Baseline#2026";
const container = process.env.DB_CONTAINER ?? "twg-phase7-db";
const database = process.env.DB_NAME ?? "twg_baseline";
const seedPath = process.env.SEED_PATH ?? "baseline/fixtures/seed.sql";

const ALPHA = {
  operasi: "33333333-3333-4333-8333-333333333331",
  skenario: "44444444-4444-4444-8444-444444444441",
  marker: "55555555-5555-4555-8555-555555555551",
};
const BETA_SCENARIO = "44444444-4444-4444-8444-444444444442";

function resetDatabase() {
  const seed = readFileSync(seedPath, "utf8");
  execFileSync(
    "docker",
    ["exec", "-i", container, "psql", "-v", "ON_ERROR_STOP=1", "-U", "postgres", "-d", database],
    { input: seed, stdio: ["pipe", "ignore", "inherit"] },
  );
}

async function http(method, path, token, device, body) {
  const headers = {};
  if (token) {
    headers.authorization = `Bearer ${token}`;
    headers["x-device-id"] = device;
  }
  if (body !== undefined) {
    headers["content-type"] = "application/json";
  }
  const response = await fetch(`${baseUrl}${path}`, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  let json = null;
  try {
    json = text ? JSON.parse(text) : null;
  } catch {
    json = null;
  }
  if (!response.ok) {
    throw new Error(`${method} ${path}: ${response.status} ${text}`);
  }
  return json;
}

async function login(username, device) {
  const response = await fetch(`${baseUrl}/auth/login`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ username, password, device_id: device }),
  });
  const json = await response.json();
  if (response.status !== 201) {
    throw new Error(`login ${username}: ${response.status} ${JSON.stringify(json)}`);
  }
  return { token: json.data.access_token, device };
}

async function ticket(session) {
  const response = await http("POST", "/private/ws-ticket", session.token, session.device);
  return response.data;
}

class WsClient {
  constructor(socket) {
    this.socket = socket;
    this.messages = [];
    this.waiters = [];
    socket.addEventListener("message", (event) => {
      let message;
      try {
        message = JSON.parse(event.data);
      } catch {
        return;
      }
      const waiter = this.waiters.find((candidate) => candidate.predicate(message));
      if (waiter) {
        clearTimeout(waiter.timer);
        this.waiters.splice(this.waiters.indexOf(waiter), 1);
        waiter.resolve(message);
      } else {
        this.messages.push(message);
      }
    });
  }

  waitFor(predicate, label = "message", timeout = 4000) {
    const index = this.messages.findIndex(predicate);
    if (index >= 0) {
      return Promise.resolve(this.messages.splice(index, 1)[0]);
    }
    return new Promise((resolve, reject) => {
      const waiter = {
        predicate,
        resolve,
        timer: setTimeout(() => {
          this.waiters.splice(this.waiters.indexOf(waiter), 1);
          reject(new Error(`timeout waiting for ${label}`));
        }, timeout),
      };
      this.waiters.push(waiter);
    });
  }

  waitType(type, timeout = 4000) {
    return this.waitFor((message) => message.type === type, type, timeout);
  }

  async expectNoEvent(timeout = 400) {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      if (this.messages.some((message) => message.type === "event")) {
        throw new Error("unexpected event received");
      }
      await new Promise((resolve) => setTimeout(resolve, 25));
    }
  }

  send(message) {
    this.socket.send(JSON.stringify(message));
  }

  close() {
    this.socket.close();
  }
}

async function connect(session) {
  const data = await ticket(session);
  if (!data.ticket || data.protocol !== "twg.ws.v1") {
    throw new Error(`invalid ticket response ${JSON.stringify(data)}`);
  }
  const url = `${baseUrl.replace(/^http/, "ws")}${data.ws_path}?ticket=${data.ticket}`;
  if (url.includes(session.token)) {
    throw new Error("access token leaked into the WebSocket URL");
  }
  const socket = new WebSocket(url);
  await new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", () => reject(new Error("websocket connect failed")), {
      once: true,
    });
  });
  const client = new WsClient(socket);
  const hello = await client.waitType("hello");
  if (hello.protocol !== "twg.ws.v1") {
    throw new Error(`unexpected hello ${JSON.stringify(hello)}`);
  }
  client.hello = hello;
  client.ticket = data.ticket;
  return client;
}

async function subscribe(client, skenarioId, lastSequence) {
  client.send({
    type: "subscribe",
    request_id: `sub-${skenarioId}`,
    skenario_id: skenarioId,
    last_sequence: lastSequence,
  });
  return client.waitFor(
    (message) =>
      (message.type === "ack" && message.request_id === `sub-${skenarioId}`) ||
      message.type === "error",
    "subscribe ack",
  );
}

async function publish(client, payload) {
  const requestId = payload.request_id ?? `pub-${Date.now()}-${Math.random()}`;
  client.send({ type: "publish", request_id: requestId, ...payload });
  return client.waitFor(
    (message) =>
      (message.type === "ack" && message.request_id === requestId) ||
      (message.type === "error" && message.request_id === requestId),
    "publish ack",
  );
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

async function main() {
  console.log("resetting database...");
  resetDatabase();

  const admin = await login("baselineadmin", "ws-admin");
  const alphaUser = await login("baselineusersatker", "ws-alpha");
  const plainUser = await login("baselineuser", "ws-user");

  const adminWs = await connect(admin);
  const alphaWs = await connect(alphaUser);
  const plainWs = await connect(plainUser);

  const adminSub = await subscribe(adminWs, ALPHA.skenario);
  assert(adminSub.type === "ack", `admin subscribe failed: ${JSON.stringify(adminSub)}`);
  const alphaSub = await subscribe(alphaWs, ALPHA.skenario);
  assert(alphaSub.type === "ack", `alpha subscribe failed: ${JSON.stringify(alphaSub)}`);

  const forbidden = await subscribe(alphaWs, BETA_SCENARIO);
  assert(
    forbidden.type === "error" && forbidden.code === "forbidden",
    `cross-satker subscribe was not rejected: ${JSON.stringify(forbidden)}`,
  );

  const plainForbidden = await subscribe(plainWs, ALPHA.skenario);
  assert(
    plainForbidden.type === "error" && plainForbidden.code === "forbidden",
    `user without satker was not rejected: ${JSON.stringify(plainForbidden)}`,
  );

  const first = await publish(adminWs, {
    operasi_id: ALPHA.operasi,
    skenario_id: ALPHA.skenario,
    marker_id: ALPHA.marker,
    command: "MOVE",
    data: JSON.stringify({ x: 3, y: 4 }),
    is_save: 1,
    idempotency_key: "smoke-move-1",
  });
  assert(first.type === "ack" && first.sequence === 1, `bad first ack ${JSON.stringify(first)}`);

  const event = await alphaWs.waitType("event");
  assert(event.command === "MOVE" && event.sequence === 1, `bad event ${JSON.stringify(event)}`);
  assert(event.satker_id === "11111111-1111-4111-8111-111111111111", "satker_id missing");

  const duplicate = await publish(adminWs, {
    operasi_id: ALPHA.operasi,
    skenario_id: ALPHA.skenario,
    marker_id: ALPHA.marker,
    command: "MOVE",
    data: JSON.stringify({ x: 3, y: 4 }),
    is_save: 0,
    idempotency_key: "smoke-move-1",
  });
  assert(
    duplicate.type === "ack" && duplicate.duplicate === true && duplicate.sequence === 1,
    `duplicate was not deduplicated: ${JSON.stringify(duplicate)}`,
  );
  await alphaWs.expectNoEvent();

  const saved = await publish(adminWs, {
    operasi_id: ALPHA.operasi,
    skenario_id: ALPHA.skenario,
    marker_id: ALPHA.marker,
    command: "ZOOM",
    data: JSON.stringify({ center: { x: 1, y: 2 }, zoom: 15, pitch: 0, bearing: 0 }),
    is_save: 1,
  });
  assert(saved.type === "ack" && saved.sequence === 2, `bad saved ack ${JSON.stringify(saved)}`);
  await alphaWs.waitFor(
    (message) => message.type === "event" && message.sequence === 2,
    "zoom event",
  );

  const lastPositions = await http(
    "GET",
    `/public/last_positions/${ALPHA.skenario}`,
    admin.token,
    admin.device,
  );
  assert(lastPositions.data.last_map.center_x === 1, "last_map.center_x not persisted");
  assert(lastPositions.data.last_map.zoom === 15, "last_map.zoom not persisted");
  assert(
    lastPositions.data.last_position.length === 1 &&
      lastPositions.data.last_position[0].pos_x === 3,
    "last_position not persisted",
  );

  adminWs.send({ type: "presence", request_id: "presence", skenario_id: ALPHA.skenario });
  const presence = await adminWs.waitFor(
    (message) => message.type === "presence" && message.request_id === "presence",
    "presence",
  );
  assert(presence.count >= 2, `presence count too small: ${JSON.stringify(presence)}`);

  adminWs.send({ type: "ping", request_id: "ping" });
  const pong = await adminWs.waitFor(
    (message) => message.type === "pong" && message.request_id === "ping",
    "pong",
  );
  assert(pong.type === "pong", "pong missing");

  const resumeWs = await connect(admin);
  const resumed = await subscribe(resumeWs, ALPHA.skenario, 1);
  assert(resumed.type === "ack", `resume subscribe failed: ${JSON.stringify(resumed)}`);
  const replayed = await resumeWs.waitFor(
    (message) => message.type === "event" && message.sequence === 2,
    "replayed event",
  );
  assert(replayed.command === "ZOOM", "resume did not replay the missing command");

  const clients = [];
  for (let index = 0; index < 8; index += 1) {
    const client = await connect(alphaUser);
    const ack = await subscribe(client, ALPHA.skenario);
    assert(ack.type === "ack", `fanout subscribe ${index} failed: ${JSON.stringify(ack)}`);
    clients.push(client);
  }
  const fanout = await publish(adminWs, {
    operasi_id: ALPHA.operasi,
    skenario_id: ALPHA.skenario,
    marker_id: ALPHA.marker,
    command: "ROT",
    data: JSON.stringify({ x: 5, y: 6, z: 7 }),
    is_save: 0,
  });
  assert(fanout.type === "ack", "fanout publish failed");
  const fanoutSequence = fanout.sequence;
  for (const client of clients) {
    const message = await client.waitFor(
      (candidate) =>
        candidate.type === "event" && candidate.sequence === fanoutSequence,
      "fanout event",
    );
    assert(message.command === "ROT", "fanout ordered payload mismatch");
  }

  const crossPublish = await publish(alphaWs, {
    operasi_id: "33333333-3333-4333-8333-333333333332",
    skenario_id: BETA_SCENARIO,
    marker_id: "55555555-5555-4555-8555-555555555553",
    command: "MOVE",
    data: JSON.stringify({ x: 1, y: 1 }),
    is_save: 0,
  });
  assert(
    crossPublish.type === "error" && crossPublish.code === "forbidden",
    `cross-satker publish was not rejected: ${JSON.stringify(crossPublish)}`,
  );

  for (let index = 0; index < 20; index += 1) {
    const ack = await publish(adminWs, {
      operasi_id: ALPHA.operasi,
      skenario_id: ALPHA.skenario,
      marker_id: ALPHA.marker,
      command: "ROT",
      data: JSON.stringify({ x: index, y: 0, z: 0 }),
      is_save: 0,
    });
    assert(ack.type === "ack", `history fill ${index} failed`);
  }

  const gapWs = await connect(admin);
  await http(
    "DELETE",
    `/private/game-sessions/snapshot/${ALPHA.skenario}`,
    admin.token,
    admin.device,
  );
  gapWs.send({
    type: "subscribe",
    request_id: "gap",
    skenario_id: ALPHA.skenario,
    last_sequence: 0,
  });
  await gapWs.waitFor(
    (message) => message.type === "ack" && message.request_id === "gap",
    "gap ack",
  );
  const gapResync = await gapWs.waitFor(
    (message) => message.type === "resync" && message.request_id === "gap",
    "gap resync",
  );
  assert(gapResync.reason === "history_gap", "resync reason mismatch");
  assert(gapResync.snapshot == null, "expected a null snapshot fallback");

  await http("PUT", `/private/game-sessions/snapshot/${ALPHA.skenario}`, admin.token, admin.device, {
    operasi_id: ALPHA.operasi,
    skenario_id: ALPHA.skenario,
    snapshot_enabled: true,
    map_state: { center: { x: 1, y: 2 } },
    markers_state: [],
    anims_state: [],
  });
  const snapshotWs = await connect(admin);
  snapshotWs.send({
    type: "subscribe",
    request_id: "snapshot",
    skenario_id: ALPHA.skenario,
    last_sequence: 0,
  });
  const snapshotResync = await snapshotWs.waitFor(
    (message) => message.type === "resync" && message.request_id === "snapshot",
    "snapshot resync",
  );
  assert(snapshotResync.snapshot?.id, "resync did not include the stored snapshot");
  snapshotWs.close();
  gapWs.close();

  const monitor = await login("baselinemonitor", "ws-monitor");
  const monitorWs = await connect(monitor);
  const monitorSub = await subscribe(monitorWs, BETA_SCENARIO);
  assert(monitorSub.type === "ack", `admin monitor subscribe failed: ${JSON.stringify(monitorSub)}`);
  monitorWs.close();

  const storm = [];
  for (let index = 0; index < 12; index += 1) {
    storm.push(
      (async () => {
        const client = await connect(plainUser);
        client.close();
      })(),
    );
  }
  await Promise.all(storm);

  for (const client of [adminWs, alphaWs, plainWs, resumeWs, ...clients]) {
    client.close();
  }

  console.log("ws smoke: PASS");
}

await main();
