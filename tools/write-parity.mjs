// Differential write-parity check: legacy Go server vs Rust server.
//
// Both servers run against the same PostgreSQL database. The script resets the
// database from baseline/fixtures/seed.sql before each pass, runs an identical
// write sequence, and compares normalized responses.
//
// Requires Docker (for the psql reset) and a fake Centrifugo endpoint because
// the legacy server publishes commands before responding.
//
//   OLD_URL=http://127.0.0.1:8013 NEW_URL=http://127.0.0.1:8011 \
//   node tools/write-parity.mjs

import { execFileSync } from "node:child_process";
import { createServer } from "node:http";
import { readFileSync } from "node:fs";

const oldUrl = (process.env.OLD_URL ?? "http://127.0.0.1:8013").replace(/\/+$/, "");
const newUrl = (process.env.NEW_URL ?? "http://127.0.0.1:8011").replace(/\/+$/, "");
const password = process.env.TWG_PASSWORD ?? "Baseline#2026";
const container = process.env.DB_CONTAINER ?? "twg-phase6-db";
const database = process.env.DB_NAME ?? "twg_baseline";
const seedPath = process.env.SEED_PATH ?? "baseline/fixtures/seed.sql";

const SEED = {
  admin: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1",
  satkerAlpha: "11111111-1111-4111-8111-111111111111",
  globalOperasi: "33333333-3333-4333-8333-333333333330",
};

const UUID_RE = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gi;
const TIME_RE = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?/g;

function resetDatabase() {
  const seed = readFileSync(seedPath, "utf8");
  execFileSync(
    "docker",
    ["exec", "-i", container, "psql", "-v", "ON_ERROR_STOP=1", "-U", "postgres", "-d", database],
    { input: seed, stdio: ["pipe", "ignore", "inherit"] },
  );
}

class Normalizer {
  constructor() {
    this.replacements = new Map();
    this.next = 1;
  }

  capture(id) {
    if (!this.replacements.has(id)) {
      this.replacements.set(id, `<id:${this.next}>`);
      this.next += 1;
    }
    return id;
  }

  normalize(value) {
    if (typeof value === "string") {
      let out = value;
      for (const [id, replacement] of this.replacements) {
        out = out.split(id).join(replacement);
      }
      out = out.replace(UUID_RE, "<uuid>");
      out = out.replace(TIME_RE, "<time>");
      out = out.replace(/\/uploads\/\d+-([^"/]+)/g, "/uploads/$1");
      out = out.replace(/v2\.local\.[A-Za-z0-9_-]+/g, "<token>");
      return out;
    }
    if (Array.isArray(value)) {
      return value.map((item) => this.normalize(item));
    }
    if (value && typeof value === "object") {
      const out = {};
      for (const [key, child] of Object.entries(value)) {
        out[key] = this.normalize(child);
      }
      return out;
    }
    return value;
  }
}

async function request(base, method, path, token, body, options = {}) {
  const headers = { ...(options.headers ?? {}) };
  if (token) {
    headers.authorization = `Bearer ${token}`;
    headers["x-device-id"] = options.deviceId ?? "parity-device";
  }
  let payload;
  if (body instanceof FormData) {
    payload = body;
  } else if (body !== undefined) {
    headers["content-type"] = "application/json";
    payload = JSON.stringify(body);
  }
  const response = await fetch(`${base}${path}`, { method, headers, body: payload });
  const text = await response.text();
  let json = null;
  try {
    json = text ? JSON.parse(text) : null;
  } catch {
    json = null;
  }
  return { status: response.status, json, text };
}

function stripFields(value, fields) {
  if (Array.isArray(value)) {
    return value.map((item) => stripFields(item, fields));
  }
  if (value && typeof value === "object") {
    const out = {};
    for (const [key, child] of Object.entries(value)) {
      if (!fields.includes(key)) {
        out[key] = stripFields(child, fields);
      }
    }
    return out;
  }
  return value;
}

async function runSuite(base, label) {
  const normalizer = new Normalizer();
  const results = [];
  const created = {};

  const check = (name, response, options = {}) => {
    if (options.capture && response.json?.data?.id) {
      created[options.capture] = response.json.data.id;
      normalizer.capture(response.json.data.id);
    }
    const body = options.statusOnly
      ? null
      : normalizer.normalize(
          options.omit ? stripFields(response.json, options.omit) : response.json,
        );
    results.push({ name: normalizer.normalize(name), status: response.status, body });
    if (options.expectStatus && response.status !== options.expectStatus) {
      throw new Error(
        `${label} ${name}: expected ${options.expectStatus}, got ${response.status} ${response.text}`,
      );
    }
    return response;
  };

  const login = await request(base, "POST", "/auth/login", null, {
    username: "baselineadmin",
    password,
    device_id: "parity-device",
  });
  check("login", login, { statusOnly: true, expectStatus: 201 });
  const token = login.json.data.access_token;
  const call = (method, path, body, options = {}) =>
    request(base, method, path, token, body, options).then((response) =>
      check(`${method} ${path}`, response, options),
    );

  // Categories -------------------------------------------------------------
  const category = await call("POST", "/admin/categories", { name: "Kategori Uji", icon: "uji" }, {
    capture: "category",
    expectStatus: 201,
  });
  await call("PUT", `/admin/categories/${created.category}`, { name: "Kategori Uji 2", icon: "uji2" });
  await call("GET", "/admin/categories?pageSize=100&pageNumber=1&search=Kategori%20Uji");

  // Satker and users -------------------------------------------------------
  const satker = await call(
    "POST",
    "/admin/satkers",
    { code: "SAT-UJI", name: "Satker Uji", is_active: true },
    { capture: "satker", expectStatus: 201 },
  );
  await call("PUT", `/admin/satkers/${created.satker}`, {
    code: "SAT-UJI",
    name: "Satker Uji 2",
    is_active: true,
  });
  const user = await call(
    "POST",
    "/admin/users",
    {
      name: "User Uji",
      username: "useruji",
      password,
      units: "[]",
      role: "user_satker",
      satker_id: created.satker,
      profile_img: "",
    },
    { capture: "user", expectStatus: 201, omit: ["password", "unit_id"] },
  );
  await call("PUT", `/admin/users/${created.user}`, {
    name: "User Uji 2",
    units: "[]",
    satker_id: created.satker,
    profile_img: "",
  }, { omit: ["password", "unit_id"] });
  await call("GET", "/private/users");

  // Operations and scenarios ----------------------------------------------
  const operation = await call(
    "POST",
    "/admin/operasis",
    { name: "Operasi Uji" },
    { capture: "operation", expectStatus: 201 },
  );
  await call("PUT", `/admin/operasis/${created.operation}`, {
    name: "Operasi Uji 2",
    satker_id: created.satker,
  });
  const scenario = await call(
    "POST",
    "/admin/skenarios",
    {
      name: "Skenario Uji",
      operasi_id: created.operation,
      center_x: 115.2,
      center_y: -8.6,
      zoom: 15,
      min_zoom: 0,
      max_zoom: 19.4,
      pitch: 0,
      bearing: 0,
      map_mode: "",
      base_layer: "",
      indoor_config: { floor: 1 },
    },
    { capture: "scenario", expectStatus: 201 },
  );
  await call("PUT", `/admin/skenarios/${created.scenario}`, {
    center_x: 115.3,
    center_y: -8.7,
    zoom: 16,
    min_zoom: 0,
    max_zoom: 20,
    pitch: 10,
    bearing: 45,
    map_mode: "indoor",
    base_layer: "grid_light",
    indoor_config: { floor: 2 },
  });
  await call("PUT", `/admin/skenarios/name/${created.scenario}`, { name: "Skenario Uji 2" });
  await call("POST", "/admin/skenarios/copy", {
    name: "Salinan Skenario",
    src_skenario_id: created.scenario,
    tgr_operasi_id: created.operation,
  });
  const scenarioList = await call("GET", `/public/skenarios/${created.operation}`);
  const copied = Array.isArray(scenarioList.json?.data)
    ? scenarioList.json.data.find((item) => item.name === "Salinan Skenario")
    : null;
  if (copied) {
    created.copiedScenario = copied.id;
    normalizer.capture(copied.id);
  }

  // Markers ----------------------------------------------------------------
  const marker = await call(
    "POST",
    "/private/markers",
    {
      name: "Marker Uji",
      operasi_id: created.operation,
      skenario_id: created.scenario,
      unit_id: SEED.admin,
      kategori: "Infanteri",
      rot_x: 0,
      rot_y: 0,
      rot_z: 0,
      pos_x: 115.2,
      pos_y: -8.6,
      jumlah: 1,
      keterangan: "awal",
    },
    { capture: "marker", expectStatus: 201 },
  );
  await call("PUT", `/private/markers/geom/${created.marker}`, {
    rot_x: 1,
    rot_y: 2,
    rot_z: 3,
    pos_x: 115.21,
    pos_y: -8.61,
  });
  await call("PUT", `/private/markers/name/${created.marker}`, {
    name: "Marker Uji 2",
    jumlah: 2,
    keterangan: "ubah",
  });
  await call("PUT", `/private/markers/scale/${created.marker}`, { scale: 2, scale_y: 3, scale_z: 4 });
  await call("PUT", `/private/markers/altitude/${created.marker}`, { altitude: 12 });
  await call("PUT", `/private/markers/rotasi/${created.marker}`, {
    rotasi: 90,
    rotasi_x: 5,
    rotasi_y: 6,
  });
  const duplicate = await call(
    "PUT",
    `/private/markers/duplicate/${created.marker}`,
    { pos_x: 115.22, pos_y: -8.62 },
    { capture: "duplicateMarker" },
  );
  await call("DELETE", `/private/markers/${created.duplicateMarker}`);

  // Buildings, lines, text markers, alurs ----------------------------------
  const building = await call(
    "POST",
    "/private/buildings",
    {
      name: "Gedung Uji",
      operasi_id: created.operation,
      skenario_id: created.scenario,
      color: "#ff0000",
      geom: "[]",
      height: 10,
      keterangan: "awal",
    },
    { capture: "building", expectStatus: 201 },
  );
  await call("PUT", `/private/buildings/${created.building}`, {
    name: "Gedung Uji 2",
    color: "#00ff00",
    keterangan: "ubah",
    height: 12,
  });
  await call("PUT", `/private/buildings/geom/${created.building}`, {
    geom: "[{\"x\":1,\"y\":2}]",
  });

  const line = await call(
    "POST",
    "/private/lines",
    {
      name: "Garis Uji",
      operasi_id: created.operation,
      skenario_id: created.scenario,
      color: "#0000ff",
      width: 2,
      alpha: 1,
      geom: "[]",
    },
    { capture: "line", expectStatus: 201 },
  );
  await call("PUT", `/private/lines/${created.line}`, {
    name: "Garis Uji 2",
    color: "#00ffff",
    width: 3,
    alpha: 0.5,
  });
  await call("PUT", `/private/lines/geom/${created.line}`, {
    geom: "[{\"x\":3,\"y\":4}]",
  });

  const textMarker = await call(
    "POST",
    "/private/text-markers",
    {
      text: "Teks Uji",
      operasi_id: created.operation,
      skenario_id: created.scenario,
      color: "#ffffff",
      font_size: 16,
      align: "center",
      wrap_text: false,
      pos_x: 115.2,
      pos_y: -8.6,
    },
    { capture: "textMarker", expectStatus: 201 },
  );
  await call("PUT", `/private/text-markers/${created.textMarker}`, {
    text: "Teks Uji 2",
    color: "#eeeeee",
    font_size: 18,
    align: "left",
    wrap_text: true,
  });
  await call("PUT", `/private/text-markers/geom/${created.textMarker}`, {
    pos_x: 115.23,
    pos_y: -8.63,
  });

  const alur = await call(
    "POST",
    "/admin/alurs",
    { operasi_id: created.operation, skenario_id: created.scenario, alur: "[]" },
    { capture: "alur", expectStatus: 201 },
  );
  await call("PUT", `/admin/alurs/${created.alur}`, { alur: "[{\"step\":1}]" });

  // Animations -------------------------------------------------------------
  const route = await call(
    "POST",
    "/private/anims/route",
    {
      operasi_id: created.operation,
      skenario_id: created.scenario,
      name: "Route Uji",
      username: "baselineadmin",
      tipe: "route",
      keterangan: "awal",
      data: "[]",
      geom: "[]",
      speed: 5,
      detik: 3,
      replay: true,
      autoplay: false,
    },
    { capture: "route", expectStatus: 201 },
  );
  await call("PUT", `/private/anims/route/data/${created.route}`, {
    name: "Route Uji 2",
    username: "baselineadmin",
    keterangan: "ubah",
    data: "[{\"x\":1,\"y\":2}]",
    speed: 6,
    detik: 4,
    replay: false,
    autoplay: true,
  });
  await call("PUT", `/private/anims/route/geom/v2/${created.route}`, {
    geom: [{ x: 1, y: 2, z: 0 }],
  });

  const spawn = await call(
    "POST",
    "/private/anims/spawn",
    {
      operasi_id: created.operation,
      skenario_id: created.scenario,
      unit_id: SEED.admin,
      kategori: "Infanteri",
      rot_x: 0,
      rot_y: 0,
      rot_z: 0,
      pos_x: 115.2,
      pos_y: -8.6,
      scale: 1,
      altitude: 3,
      marker_name: "Marker Spawn",
      name: "Spawn Uji",
      username: "baselineadmin",
      keterangan: "awal",
    },
    { capture: "spawn", expectStatus: 201 },
  );
  await call("PUT", `/private/anims/spawn/data/${created.spawn}`, {
    name: "Spawn Uji 2",
    username: "baselineadmin",
    keterangan: "ubah",
    data: [],
  });

  const secondMarker = await call(
    "POST",
    "/private/markers",
    {
      name: "Marker Pickup",
      operasi_id: created.operation,
      skenario_id: created.scenario,
      unit_id: SEED.admin,
      kategori: "Infanteri",
      rot_x: 0,
      rot_y: 0,
      rot_z: 0,
      pos_x: 115.2,
      pos_y: -8.6,
      jumlah: 1,
      keterangan: "",
    },
    { capture: "secondMarker", expectStatus: 201 },
  );
  const pickup = await call(
    "POST",
    "/private/anims/spawn_pickup",
    {
      operasi_id: created.operation,
      skenario_id: created.scenario,
      marker_ids: [created.secondMarker],
      name: "Pickup Uji",
      username: "baselineadmin",
      keterangan: "",
    },
    { capture: "pickup", expectStatus: 201 },
  );
  await call("DELETE", `/private/anims/spawn/${created.pickup}`);

  const remove = await call(
    "POST",
    "/private/anims/remove",
    {
      operasi_id: created.operation,
      skenario_id: created.scenario,
      name: "Remove Uji",
      username: "baselineadmin",
      keterangan: "",
      data: "[]",
    },
    { capture: "remove", expectStatus: 201 },
  );
  await call("PUT", `/private/anims/remove/data/${created.remove}`, {
    name: "Remove Uji 2",
    username: "baselineadmin",
    keterangan: "ubah",
    data: "[]",
  });

  const combine = await call(
    "POST",
    "/private/anims/combine/v2",
    {
      operasi_id: created.operation,
      skenario_id: created.scenario,
      name: "Combine Uji",
      username: "baselineadmin",
      keterangan: "",
      data: [{ id: created.route, tipe: "route" }],
      tipe: "combine",
    },
    { capture: "combine", expectStatus: 201 },
  );
  await call("PUT", `/private/anims/combine/data/${created.combine}`, {
    name: "Combine Uji 2",
    username: "baselineadmin",
    keterangan: "ubah",
    data: JSON.stringify([{ id: created.route, tipe: "route" }]),
  });
  await call("PUT", `/private/anims/combine/delete/${created.combine}`, { tipe: "route" });
  await call("DELETE", `/private/anims/combine/${created.combine}`);

  const shootMarker = await call(
    "POST",
    "/private/markers",
    {
      name: "Marker Shoot",
      operasi_id: created.operation,
      skenario_id: created.scenario,
      unit_id: SEED.admin,
      kategori: "Infanteri",
      rot_x: 0,
      rot_y: 0,
      rot_z: 0,
      pos_x: 115.2,
      pos_y: -8.6,
      jumlah: 1,
      keterangan: "",
    },
    { capture: "shootMarker", expectStatus: 201 },
  );
  const shootData = {
    marker_id: created.shootMarker,
    scale: 1,
    height: 1,
    dir_forward: 1,
    spread: 1,
    speed: 1,
    angle: 1,
    smoke_count: 1,
    tipe: "senapan",
  };
  const shoot = await call(
    "POST",
    "/private/anims/shoot/v2",
    {
      operasi_id: created.operation,
      skenario_id: created.scenario,
      name: "Shoot Uji",
      username: "baselineadmin",
      keterangan: "",
      data: shootData,
    },
    { capture: "shoot", expectStatus: 201 },
  );
  await call("PUT", `/private/anims/shoot/data/${created.shoot}`, {
    name: "Shoot Uji 2",
    username: "baselineadmin",
    keterangan: "ubah",
    data: JSON.stringify({ ...shootData, smoke_count: 2 }),
  });
  await call("DELETE", `/private/anims/shoot/${created.shoot}`);

  const particle = await call(
    "POST",
    "/private/anims/spawn_particle",
    {
      operasi_id: created.operation,
      skenario_id: created.scenario,
      name: "Partikel Uji",
      username: "baselineadmin",
      keterangan: "",
      data: "{}",
      tipe: "fire",
      is_spawn: "true",
      pos_x: 115.2,
      pos_y: -8.6,
    },
    { capture: "particle", expectStatus: 201 },
  );
  await call("PUT", `/private/anims/particle/data/${created.particle}`, {
    name: "Partikel Uji 2",
    username: "baselineadmin",
    keterangan: "ubah",
    tipe: "smoke",
    data: "{\"a\":1}",
  });
  await call("DELETE", `/private/anims/particle/${created.particle}`);

  // Snapshot ---------------------------------------------------------------
  await call("PUT", `/private/game-sessions/snapshot/${created.scenario}`, {
    operasi_id: created.operation,
    skenario_id: created.scenario,
    snapshot_enabled: true,
    map_state: { center: { x: 1, y: 2 } },
    markers_state: [{ id: created.marker }],
    anims_state: [],
  });
  await call(
    "PATCH",
    `/private/game-sessions/snapshot/${created.scenario}/enabled`,
    { snapshot_enabled: false },
  );
  await call("GET", `/private/game-sessions/snapshot/${created.scenario}`);
  await call("DELETE", `/private/game-sessions/snapshot/${created.scenario}`);

  // Commands / last positions ---------------------------------------------
  await call("POST", "/private/socket", {
    operasi_id: created.operation,
    skenario_id: created.scenario,
    marker_id: created.marker,
    command: "ZOOM",
    data: JSON.stringify({ center: { x: 1, y: 2 }, zoom: 15, pitch: 0, bearing: 0 }),
    is_save: 1,
  }, { statusOnly: true });
  await call("POST", "/private/socket", {
    operasi_id: created.operation,
    skenario_id: created.scenario,
    marker_id: created.marker,
    command: "MOVE",
    data: JSON.stringify({ x: 3, y: 4 }),
    is_save: 1,
  }, { statusOnly: true });
  await call("POST", "/private/socket", {
    operasi_id: created.operation,
    skenario_id: created.scenario,
    marker_id: created.marker,
    command: "ROT",
    data: JSON.stringify({ x: 5, y: 6, z: 7 }),
    is_save: 1,
  }, { statusOnly: true });
  await call("GET", `/public/last_positions/${created.scenario}`);

  // Reads ------------------------------------------------------------------
  await call("GET", `/public/skenarios/${created.operation}`);
  await call("GET", `/public/markers/list/${created.scenario}`);
  await call("GET", `/public/buildings/list/${created.scenario}`);
  await call("GET", `/public/lines/list/${created.scenario}`);
  await call("GET", `/public/text-markers/list/${created.scenario}`);
  await call("GET", `/public/alurs/list/${created.scenario}`);
  await call("GET", `/public/anims/list/${created.scenario}`);
  await call("GET", `/public/list/operasis`);

  // Upload -----------------------------------------------------------------
  const form = new FormData();
  const png = Buffer.from(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
    "base64",
  );
  form.append("image", new Blob([png], { type: "image/png" }), "pixel.png");
  await call("POST", "/private/upload-image", form);

  // Deletes ----------------------------------------------------------------
  await call("DELETE", `/admin/alurs/${created.alur}`);
  await call("DELETE", `/private/text-markers/${created.textMarker}`);
  await call("DELETE", `/private/lines/${created.line}`);
  await call("DELETE", `/private/buildings/${created.building}`);
  await call("DELETE", `/private/markers/${created.shootMarker}`);
  await call("DELETE", `/private/markers/${created.secondMarker}`);
  await call("DELETE", `/private/markers/${created.marker}`);
  await call("DELETE", `/private/anims/remove/${created.remove}`);
  await call("DELETE", `/private/anims/${created.route}`);
  await call("DELETE", `/private/anims/spawn/${created.spawn}`);
  if (created.copiedScenario) {
    await call("DELETE", `/admin/skenarios/${created.copiedScenario}`);
  }
  await call("DELETE", `/admin/skenarios/${created.scenario}`);
  await call("DELETE", `/admin/operasis/${created.operation}`);
  await call("DELETE", `/admin/users/${created.user}`);
  await call("DELETE", `/admin/satkers/${created.satker}`);
  await call("DELETE", `/admin/categories/${created.category}`);

  return results;
}

async function runSafetyChecks(base) {
  const results = [];
  const login = await request(base, "POST", "/auth/login", null, {
    username: "baselineadmin",
    password,
    device_id: "parity-device",
  });
  if (login.status !== 201) {
    throw new Error(`safety login failed: ${login.status} ${login.text}`);
  }
  const token = login.json.data.access_token;
  const expect = async (name, method, path, body, expected) => {
    const response = await request(base, method, path, token, body);
    results.push({ name, status: response.status });
    if (response.status !== expected) {
      throw new Error(`${name}: expected ${expected}, got ${response.status} ${response.text}`);
    }
  };

  await expect(
    "duplicate username",
    "POST",
    "/admin/users",
    {
      name: "Duplikat",
      username: "baselineadmin",
      password,
      units: "[]",
      role: "user",
      profile_img: "",
    },
    409,
  );
  await expect(
    "missing marker",
    "PUT",
    "/private/markers/geom/99999999-9999-4999-8999-999999999999",
    { rot_x: 0, rot_y: 0, rot_z: 0, pos_x: 1, pos_y: 2 },
    404,
  );
  await expect(
    "invalid marker id path",
    "PUT",
    "/private/markers/geom/not-a-uuid",
    { rot_x: 0, rot_y: 0, rot_z: 0, pos_x: 1, pos_y: 2 },
    400,
  );
  await expect("missing category", "DELETE", "/admin/categories/99999999-9999-4999-8999-999999999999", undefined, 404);
  await expect(
    "empty combine body",
    "POST",
    "/private/anims/combine/v2",
    {
      operasi_id: SEED.globalOperasi,
      skenario_id: "44444444-4444-4444-8444-444444444440",
      name: "Kosong",
      username: "baselineadmin",
      data: [],
    },
    400,
  );
  await expect(
    "toggle missing snapshot",
    "PATCH",
    "/private/game-sessions/snapshot/44444444-4444-4444-8444-444444444440/enabled",
    { snapshot_enabled: true },
    404,
  );

  const badForm = new FormData();
  badForm.append("image", new Blob(["not an image"], { type: "text/plain" }), "note.txt");
  await expect("invalid upload mime", "POST", "/private/upload-image", badForm, 400);

  const missingField = new FormData();
  missingField.append("other", new Blob(["x"], { type: "text/plain" }), "note.txt");
  await expect("missing upload field", "POST", "/private/upload-image", missingField, 400);

  return results;
}

function canonical(value) {
  if (Array.isArray(value)) {
    return value.map((item) => canonical(item));
  }
  if (value && typeof value === "object") {
    const out = {};
    for (const key of Object.keys(value).sort()) {
      out[key] = canonical(value[key]);
    }
    return out;
  }
  return value;
}

function firstDifference(oldResults, newResults) {
  const length = Math.max(oldResults.length, newResults.length);
  for (let index = 0; index < length; index += 1) {
    const oldResult = oldResults[index];
    const newResult = newResults[index];
    if (!oldResult || !newResult) {
      return { index, oldResult, newResult, reason: "missing result" };
    }
    if (oldResult.name !== newResult.name) {
      return { index, oldResult, newResult, reason: "name" };
    }
    if (oldResult.status !== newResult.status) {
      return { index, oldResult, newResult, reason: "status" };
    }
    const oldBody = JSON.stringify(canonical(oldResult.body));
    const newBody = JSON.stringify(canonical(newResult.body));
    if (oldBody !== newBody) {
      return { index, oldResult, newResult, reason: "body" };
    }
  }
  return null;
}

const centrifuge = createServer((request, response) => {
  request.resume();
  request.on("end", () => {
    response.writeHead(200, { "content-type": "application/json" });
    response.end('{"result":{}}');
  });
});
await new Promise((resolve) => centrifuge.listen(8000, "127.0.0.1", resolve));

try {
  console.log("resetting database for legacy server...");
  resetDatabase();
  console.log("running legacy suite...");
  const oldResults = await runSuite(oldUrl, "legacy");
  console.log(`legacy checks: ${oldResults.length}`);

  console.log("resetting database for rust server...");
  resetDatabase();
  console.log("running rust suite...");
  const newResults = await runSuite(newUrl, "rust");
  console.log(`rust checks: ${newResults.length}`);

  console.log("running rust safety checks...");
  const safetyResults = await runSafetyChecks(newUrl);
  console.log(`safety checks: ${safetyResults.length}`);

  const difference = firstDifference(oldResults, newResults);
  if (difference) {
    console.error(`WRITE PARITY: FAIL at check #${difference.index + 1}`);
    console.error("reason:", difference.reason);
    console.error("legacy:", JSON.stringify(difference.oldResult, null, 2));
    console.error("rust:", JSON.stringify(difference.newResult, null, 2));
    process.exitCode = 1;
  } else {
    console.log(
      `write parity: PASS (${oldResults.length} checks, ${safetyResults.length} safety checks)`,
    );
  }
} finally {
  centrifuge.close();
}
