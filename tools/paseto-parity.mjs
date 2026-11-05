// Bidirectional PASETO/session parity check between the legacy Go server and
// the Rust server. Both must point at the same baseline database and use the
// same TOKEN_SYMMETRIC_KEY.
//
//   OLD_URL=http://127.0.0.1:8013 NEW_URL=http://127.0.0.1:8011 \
//   node tools/paseto-parity.mjs

const oldUrl = (process.env.OLD_URL ?? "http://127.0.0.1:8013").replace(/\/+$/, "");
const newUrl = (process.env.NEW_URL ?? "http://127.0.0.1:8011").replace(/\/+$/, "");
const username = process.env.TWG_USERNAME ?? "baselineadmin";
const password = process.env.TWG_PASSWORD ?? "Baseline#2026";
const deviceId = process.env.TWG_DEVICE_ID ?? "device-parity";
let failures = 0;

function check(name, condition, detail = "") {
  if (condition) {
    console.log(`PASS ${name}`);
  } else {
    failures += 1;
    console.error(`FAIL ${name}${detail ? `: ${detail}` : ""}`);
  }
}

async function request(base, path, options = {}) {
  const response = await fetch(`${base}${path}`, options);
  const text = await response.text();
  let body = null;
  try {
    body = JSON.parse(text);
  } catch {
    body = text;
  }
  return { status: response.status, body };
}

async function login(base) {
  return request(base, "/auth/login", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ username, password, device_id: deviceId }),
  });
}

async function identify(base, accessToken) {
  return request(base, "/private/identify", {
    headers: {
      authorization: `Bearer ${accessToken}`,
      "x-device-id": deviceId,
    },
  });
}

async function refresh(base, refreshToken) {
  return request(base, "/auth/refresh", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ refresh_token: refreshToken, device_id: deviceId }),
  });
}

const oldLogin = await login(oldUrl);
check(
  "legacy login",
  oldLogin.status === 201 && oldLogin.body?.code === 200,
  JSON.stringify(oldLogin.body),
);
const oldAccess = oldLogin.body?.data?.access_token;
const oldRefresh = oldLogin.body?.data?.refresh_token;

const rustRefreshWithOldRefresh = await refresh(newUrl, oldRefresh);
check(
  "Rust accepts legacy refresh token",
  rustRefreshWithOldRefresh.status === 200,
  JSON.stringify(rustRefreshWithOldRefresh.body),
);

const rustIdentifyWithOldAccess = await identify(newUrl, oldAccess);
check(
  "Rust accepts legacy access token",
  rustIdentifyWithOldAccess.status === 200,
  JSON.stringify(rustIdentifyWithOldAccess.body),
);

const rustLogin = await login(newUrl);
check(
  "rust login",
  rustLogin.status === 201 && rustLogin.body?.code === 200,
  JSON.stringify(rustLogin.body),
);
const rustAccess = rustLogin.body?.data?.access_token;
const rustRefresh = rustLogin.body?.data?.refresh_token;

const legacyIdentifyWithRustAccess = await identify(oldUrl, rustAccess);
check(
  "legacy server accepts Rust access token",
  legacyIdentifyWithRustAccess.status === 200,
  JSON.stringify(legacyIdentifyWithRustAccess.body),
);

const legacyRefreshWithRustRefresh = await refresh(oldUrl, rustRefresh);
check(
  "legacy server accepts Rust refresh token",
  legacyRefreshWithRustRefresh.status === 200,
  JSON.stringify(legacyRefreshWithRustRefresh.body),
);

if (failures > 0) {
  console.error(`${failures} parity check(s) failed`);
  process.exit(1);
}
console.log("PASETO/session parity: PASS");
