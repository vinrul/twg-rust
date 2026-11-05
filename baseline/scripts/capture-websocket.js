const fs = require("fs");
const path = require("path");
const { Centrifuge } = require("centrifuge");

const BASE_URL = process.env.BASE_URL || "http://127.0.0.1:8010";
const CENTRIFUGO_URL =
  process.env.CENTRIFUGO_URL || "ws://127.0.0.1:8000/connection/websocket";
const CHANNEL = process.env.CHANNEL || "twg-baseline";
const OUT_DIR =
  process.env.OUT_DIR || path.join(__dirname, "..", "golden");
const TIMEOUT_MS = Number(process.env.TIMEOUT_MS || 10000);

const IDS = {
  operasiAlpha: "33333333-3333-4333-8333-333333333331",
  skenarioAlpha: "44444444-4444-4444-8444-444444444441",
  markerAlpha: "55555555-5555-4555-8555-555555555551",
  skenarioBeta: "44444444-4444-4444-8444-444444444442",
  markerBeta: "55555555-5555-4555-8555-555555555553",
};

function withTimeout(promise, label) {
  return Promise.race([
    promise,
    new Promise((_, reject) =>
      setTimeout(() => reject(new Error(`Timeout waiting for ${label}`)), TIMEOUT_MS),
    ),
  ]);
}

async function request(method, url, body, headers = {}) {
  const response = await fetch(url, {
    method,
    headers: {
      Accept: "application/json",
      ...(body ? { "Content-Type": "application/json" } : {}),
      ...headers,
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await response.text();
  let json = null;
  try {
    json = text ? JSON.parse(text) : null;
  } catch {
    json = { raw: text };
  }
  return { status: response.status, body: json, raw: text };
}

async function login() {
  const deviceId = "baseline-admin_satker-device";
  const response = await request("POST", `${BASE_URL}/auth/login`, {
    username: "baselineadminsatker",
    password: "Baseline#2026",
    device_id: deviceId,
  });
  if (response.status !== 201 || !response.body?.data?.access_token) {
    throw new Error(`Login failed: HTTP ${response.status}`);
  }
  return { token: response.body.data.access_token, deviceId };
}

async function main() {
  fs.mkdirSync(OUT_DIR, { recursive: true });

  const results = {
    captured_at: new Date().toISOString(),
    base_url: BASE_URL,
    centrifugo_url: CENTRIFUGO_URL,
    channel: CHANNEL,
    login: null,
    subscribed: false,
    publish_allowed: null,
    publication: null,
    cross_satker: null,
    errors: [],
  };

  let centrifuge;
  let subscription;

  try {
    const session = await login();
    results.login = {
      username: "baselineadminsatker",
      role: "admin_satker",
      device_id: session.deviceId,
      http_status: 201,
    };

    centrifuge = new Centrifuge(CENTRIFUGO_URL, { debug: false });
    subscription = centrifuge.newSubscription(CHANNEL);

    const subscribedPromise = new Promise((resolve, reject) => {
      subscription.on("subscribed", resolve);
      subscription.on("error", (ctx) =>
        reject(new Error(`Subscription error: ${JSON.stringify(ctx)}`)),
      );
    });
    centrifuge.on("error", (ctx) =>
      results.errors.push(`Centrifuge error: ${JSON.stringify(ctx)}`),
    );

    subscription.subscribe();
    centrifuge.connect();
    await withTimeout(subscribedPromise, "subscription");
    results.subscribed = true;

    const publicationPromise = new Promise((resolve, reject) => {
      subscription.on("publication", (ctx) => {
        if (ctx?.data?.data?.includes("ws-baseline")) {
          resolve(ctx);
        }
      });
      setTimeout(
        () => reject(new Error("Timeout waiting for publication")),
        TIMEOUT_MS,
      );
    });

    const allowed = await request(
      "POST",
      `${BASE_URL}/private/socket`,
      {
        command: "MOVE",
        data: JSON.stringify({
          x: 115.2251,
          y: -8.6481,
          z: 0,
          source: "ws-baseline",
        }),
        skenario_id: IDS.skenarioAlpha,
        operasi_id: IDS.operasiAlpha,
        marker_id: IDS.markerAlpha,
        is_save: 1,
      },
      {
        Authorization: `Bearer ${session.token}`,
        "X-Device-ID": session.deviceId,
      },
    );
    results.publish_allowed = {
      http_status: allowed.status,
      body: allowed.body,
    };
    if (allowed.status !== 201) {
      throw new Error(`Authorized publish failed: HTTP ${allowed.status}`);
    }

    const publication = await withTimeout(
      publicationPromise,
      "publication payload",
    );
    results.publication = publication;

    const denied = await request(
      "POST",
      `${BASE_URL}/private/socket`,
      {
        command: "MOVE",
        data: JSON.stringify({ x: 115.23, y: -8.65, source: "ws-cross-satker" }),
        skenario_id: IDS.skenarioBeta,
        operasi_id: "33333333-3333-4333-8333-333333333332",
        marker_id: IDS.markerBeta,
        is_save: 0,
      },
      {
        Authorization: `Bearer ${session.token}`,
        "X-Device-ID": session.deviceId,
      },
    );
    results.cross_satker = {
      http_status: denied.status,
      body: denied.body,
    };

    if (denied.status !== 403) {
      throw new Error(`Cross-satker publish was not denied: HTTP ${denied.status}`);
    }
  } catch (error) {
    results.errors.push(error.message);
  } finally {
    if (subscription) {
      try {
        subscription.unsubscribe();
      } catch {}
    }
    if (centrifuge) {
      try {
        centrifuge.disconnect();
      } catch {}
    }
  }

  const jsonPath = path.join(OUT_DIR, "websocket-capture.json");
  fs.writeFileSync(jsonPath, JSON.stringify(results, null, 2) + "\n", "utf8");

  const lines = [
    "# WebSocket Baseline",
    "",
    `Captured at: ${results.captured_at}`,
    "",
    `- Channel: \`${CHANNEL}\``,
    `- Subscribed: **${results.subscribed}**`,
    `- Allowed publish HTTP: **${results.publish_allowed?.http_status ?? "n/a"}**`,
    `- Cross-satker publish HTTP: **${results.cross_satker?.http_status ?? "n/a"}**`,
    `- Errors: **${results.errors.length}**`,
    "",
  ];
  if (results.publication) {
    lines.push("## Publication diterima");
    lines.push("");
    lines.push("```json");
    lines.push(JSON.stringify(results.publication.data, null, 2));
    lines.push("```");
    lines.push("");
  }
  if (results.cross_satker) {
    lines.push("## Cross-satker response");
    lines.push("");
    lines.push("```json");
    lines.push(JSON.stringify(results.cross_satker, null, 2));
    lines.push("```");
  }
  if (results.errors.length > 0) {
    lines.push("");
    lines.push("## Errors");
    lines.push("");
    for (const error of results.errors) {
      lines.push(`- ${error}`);
    }
  }

  fs.writeFileSync(
    path.join(OUT_DIR, "websocket-capture.generated.md"),
    lines.join("\n") + "\n",
    "utf8",
  );

  console.log(JSON.stringify(results, null, 2));
  if (results.errors.length > 0) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
