// Phase 4 auth smoke test. Requires a running server and the baseline user.
//
//   BASE_URL=http://127.0.0.1:8011 \
//   TWG_USERNAME=baselineadmin TWG_PASSWORD='Baseline#2026' \
//   node tools/phase4-smoke.mjs

const baseUrl = (process.env.BASE_URL ?? "http://127.0.0.1:8011").replace(/\/+$/, "");
const username = process.env.TWG_USERNAME ?? "baselineadmin";
const password = process.env.TWG_PASSWORD ?? "Baseline#2026";
const deviceId = process.env.TWG_DEVICE_ID ?? "device-smoke";

let failures = 0;

function check(name, condition, detail = "") {
  if (condition) {
    console.log(`PASS ${name}`);
  } else {
    failures += 1;
    console.error(`FAIL ${name}${detail ? `: ${detail}` : ""}`);
  }
}

async function request(path, options = {}) {
  const response = await fetch(`${baseUrl}${path}`, options);
  const text = await response.text();
  let body = null;
  try {
    body = JSON.parse(text);
  } catch {
    body = text;
  }
  return { status: response.status, headers: response.headers, body };
}

function json(body) {
  return {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  };
}

const health = await request("/healthz");
check("healthz", health.status === 200 && health.body?.status === "ok", JSON.stringify(health));

const ready = await request("/readyz");
check("readyz", ready.status === 200 && ready.body?.status === "ok", JSON.stringify(ready));

const login = await request(
  "/auth/login",
  json({ username, password, device_id: deviceId }),
);
check("login status", login.status === 201, String(login.status));
check("login envelope", login.body?.code === 200 && login.body?.message === "success");
const accessToken = login.body?.data?.access_token;
const refreshToken = login.body?.data?.refresh_token;
check("login tokens", accessToken?.startsWith("v2.local.") && refreshToken?.startsWith("v2.local."));

const secondDevice = await request(
  "/auth/login",
  json({ username, password, device_id: "device-smoke-2" }),
);
check(
  "single-device conflict",
  secondDevice.status === 403 && secondDevice.body?.message === "already logged in on another device",
  JSON.stringify(secondDevice.body),
);

const identify = await request("/private/identify", {
  headers: {
    authorization: `Bearer ${accessToken}`,
    "x-device-id": deviceId,
  },
});
check("identify", identify.status === 200 && identify.body?.code === 200, JSON.stringify(identify.body));
const identifiedRefresh = identify.body?.data?.refresh_token;
const identifiedAccess = identify.body?.data?.access_token;
check(
  "identify rotates refresh",
  identifiedRefresh && identifiedRefresh !== refreshToken,
);

const oldRefresh = await request(
  "/auth/refresh",
  json({ refresh_token: refreshToken, device_id: deviceId }),
);
check("pre-identify refresh rejected", oldRefresh.status === 401, JSON.stringify(oldRefresh.body));

const refreshed = await request(
  "/auth/refresh",
  json({ refresh_token: identifiedRefresh, device_id: deviceId }),
);
check("refresh", refreshed.status === 200, JSON.stringify(refreshed.body));
const nextRefresh = refreshed.body?.data?.refresh_token;
const nextAccess = refreshed.body?.data?.access_token ?? identifiedAccess;
check("refresh rotates", nextRefresh && nextRefresh !== identifiedRefresh);

const replay = await request(
  "/auth/refresh",
  json({ refresh_token: identifiedRefresh, device_id: deviceId }),
);
check("refresh replay rejected", replay.status === 401, JSON.stringify(replay.body));

const logout = await request("/auth/logout", {
  method: "POST",
  headers: {
    authorization: `Bearer ${nextAccess}`,
    "content-type": "application/json",
  },
  body: JSON.stringify({ device_id: deviceId }),
});
check("logout", logout.status === 200 && logout.body?.message === "success", JSON.stringify(logout.body));

const revoked = await request("/private/identify", {
  headers: {
    authorization: `Bearer ${nextAccess}`,
    "x-device-id": deviceId,
  },
});
check(
  "session revoked",
  revoked.status === 401 && revoked.body?.message === "session revoked",
  JSON.stringify(revoked.body),
);

const wrongPassword = await request(
  "/auth/login",
  json({ username, password: "wrong-password", device_id: deviceId }),
);
check(
  "wrong password",
  wrongPassword.status === 401 && wrongPassword.body?.message === "invalid password",
  JSON.stringify(wrongPassword.body),
);

if (failures > 0) {
  console.error(`${failures} check(s) failed`);
  process.exit(1);
}
console.log("phase 4 auth smoke: PASS");
