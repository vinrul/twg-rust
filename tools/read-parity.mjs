// Phase 5 read-only differential test.
//
// Both servers must point at the same seeded baseline database and use the same
// TOKEN_SYMMETRIC_KEY. The script logs in through the Rust server, then sends
// the same token/device to both servers and compares status + JSON body.
//
//   OLD_URL=http://127.0.0.1:8013 NEW_URL=http://127.0.0.1:8011 \
//   node tools/read-parity.mjs

import { readFile } from "node:fs/promises";

const oldUrl = (process.env.OLD_URL ?? "http://127.0.0.1:8013").replace(/\/+$/, "");
const newUrl = (process.env.NEW_URL ?? "http://127.0.0.1:8011").replace(/\/+$/, "");
const password = process.env.TWG_PASSWORD ?? "Baseline#2026";
const goldenDir = new URL("../baseline/golden/redacted/", import.meta.url);

const scenarioId = "44444444-4444-4444-8444-444444444441";
const emptyScenarioId = "44444444-4444-4444-8444-444444444443";
const operationId = "33333333-3333-4333-8333-333333333331";
const categoryId = "c0000000-0000-4000-8000-000000000001";

const publicPaths = [
  `/public/skenarios/${operationId}`,
  `/public/skenarios/get/${scenarioId}`,
  `/public/markers/list/${scenarioId}`,
  `/public/text-markers/list/${scenarioId}`,
  `/public/particles/list/${scenarioId}`,
  `/public/buildings/list/${scenarioId}`,
  `/public/lines/list/${scenarioId}`,
  `/public/alurs/list/${scenarioId}`,
  `/public/anims/list/${scenarioId}`,
  `/public/last_positions/${scenarioId}`,
  "/public/list/operasis",
  `/public/get/home/operasis/${operationId}`,
  "/public/users/online",
  `/public/game-sessions/snapshot/${scenarioId}`,
  `/public/markers/list/${emptyScenarioId}`,
  `/public/text-markers/list/${emptyScenarioId}`,
  `/public/particles/list/${emptyScenarioId}`,
  `/public/buildings/list/${emptyScenarioId}`,
  `/public/lines/list/${emptyScenarioId}`,
  `/public/alurs/list/${emptyScenarioId}`,
  `/public/anims/list/${emptyScenarioId}`,
  `/public/last_positions/${emptyScenarioId}`,
  `/public/game-sessions/snapshot/${emptyScenarioId}`,
];

const privatePaths = [
  "/private/operasis?pageSize=10&pageNumber=1&search=",
  "/private/operasis?pageSize=1&pageNumber=2&search=",
  "/private/operasis?pageSize=10&pageNumber=1&search=Alpha",
  "/private/operasis?pageSize=1&pageNumber=10&search=",
  "/private/operasis?pageSize=10&pageNumber=1&search=zzz",
  `/private/operasis/${operationId}`,
  "/private/home/operasis",
  "/private/users",
  "/private/satkers",
  "/private/satkers?active=true",
  "/private/categories/all",
  `/private/game-sessions/snapshot/${scenarioId}`,
];

const adminPaths = [
  "/admin/categories?pageSize=10&pageNumber=1&search=",
  "/admin/categories?pageSize=1&pageNumber=2&search=",
  "/admin/categories?pageSize=10&pageNumber=1&search=Infan",
  "/admin/categories?pageSize=1&pageNumber=10&search=",
  "/admin/categories?pageSize=10&pageNumber=1&search=zzz",
  `/admin/categories/${categoryId}`,
];

let failures = 0;
let checks = 0;
let skipped = 0;

function normalize(value) {
  if (Array.isArray(value)) {
    return value.map(normalize);
  }
  if (value && typeof value === "object") {
    const result = {};
    for (const key of Object.keys(value).sort()) {
      result[key] = key.endsWith("_at") ? "<timestamp>" : normalize(value[key]);
    }
    return result;
  }
  return value;
}

function check(name, condition, detail = "") {
  checks += 1;
  if (condition) {
    console.log(`PASS ${name}`);
  } else {
    failures += 1;
    console.error(`FAIL ${name}${detail ? `: ${detail}` : ""}`);
  }
}

function skip(name, reason) {
  skipped += 1;
  console.log(`SKIP ${name}: ${reason}`);
}

function canonical(value) {
  if (Array.isArray(value)) {
    return value.map(canonical);
  }
  if (value && typeof value === "object") {
    const result = {};
    for (const key of Object.keys(value).sort()) {
      result[key] = canonical(value[key]);
    }
    return result;
  }
  return value;
}

function goldenFile(path, roleKey) {
  const publicFiles = {
    [`/public/skenarios/${operationId}`]:
      "public_public_skenarios_33333333_3333_4333_8333_333333333331.json",
    [`/public/skenarios/get/${scenarioId}`]:
      "public_public_skenarios_get_44444444_4444_4444_8444_444444444441.json",
    [`/public/markers/list/${scenarioId}`]:
      "public_public_markers_list_44444444_4444_4444_8444_444444444441.json",
    [`/public/text-markers/list/${scenarioId}`]:
      "public_public_text_markers_list_44444444_4444_4444_8444_444444444441.json",
    [`/public/particles/list/${scenarioId}`]:
      "public_public_particles_list_44444444_4444_4444_8444_444444444441.json",
    [`/public/buildings/list/${scenarioId}`]:
      "public_public_buildings_list_44444444_4444_4444_8444_444444444441.json",
    [`/public/lines/list/${scenarioId}`]:
      "public_public_lines_list_44444444_4444_4444_8444_444444444441.json",
    [`/public/alurs/list/${scenarioId}`]:
      "public_public_alurs_list_44444444_4444_4444_8444_444444444441.json",
    [`/public/anims/list/${scenarioId}`]:
      "public_public_anims_list_44444444_4444_4444_8444_444444444441.json",
    [`/public/last_positions/${scenarioId}`]:
      "public_public_last_positions_44444444_4444_4444_8444_444444444441.json",
    "/public/list/operasis": "public_public_list_operasis.json",
    [`/public/get/home/operasis/${operationId}`]:
      "public_public_get_home_operasis_33333333_3333_4333_8333_333333333331.json",
    [`/public/game-sessions/snapshot/${scenarioId}`]:
      "public_public_game_sessions_snapshot_44444444_4444_4444_8444_444444444441.json",
  };
  if (publicFiles[path]) {
    return publicFiles[path];
  }
  if (!roleKey) {
    return null;
  }
  const prefix = `private-${roleKey}-`;
  const privateFiles = {
    "/private/operasis?pageSize=10&pageNumber=1&search=": `${prefix}_private_operasis_pageSize_10_pageNumber_1_search_.json`,
    [`/private/operasis/${operationId}`]: `${prefix}_private_operasis_33333333_3333_4333_8333_333333333331.json`,
    "/private/home/operasis": `${prefix}_private_home_operasis.json`,
    "/private/users": `${prefix}_private_users.json`,
    "/private/satkers": `${prefix}_private_satkers.json`,
    "/private/categories/all": `${prefix}_private_categories_all.json`,
    [`/private/game-sessions/snapshot/${scenarioId}`]: `${prefix}_private_game_sessions_snapshot_44444444_4444_4444_8444_444444444441.json`,
  };
  if (privateFiles[path]) {
    return privateFiles[path];
  }
  if (roleKey === "admin" && path === "/admin/categories?pageSize=10&pageNumber=1&search=") {
    return "admin-admin-_admin_categories_pageSize_10_pageNumber_1_search_.json";
  }
  return null;
}

async function request(base, path, headers = {}) {
  const response = await fetch(`${base}${path}`, { headers });
  const text = await response.text();
  let body = null;
  try {
    body = JSON.parse(text);
  } catch {
    body = text;
  }
  return {
    status: response.status,
    body,
    contentType: (response.headers.get("content-type") ?? "").split(";")[0],
    requestId: response.headers.get("x-request-id"),
  };
}

async function login(username, deviceId) {
  const response = await fetch(`${newUrl}/auth/login`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ username, password, device_id: deviceId }),
  });
  const body = await response.json();
  if (response.status !== 201) {
    throw new Error(`login ${username} failed: ${response.status} ${JSON.stringify(body)}`);
  }
  return body.data.access_token;
}

async function comparePath(path, token, deviceId, roleKey = null) {
  const headers = token
    ? { authorization: `Bearer ${token}`, "x-device-id": deviceId }
    : {};
  const [oldResponse, newResponse] = await Promise.all([
    request(oldUrl, path, headers),
    request(newUrl, path, headers),
  ]);
  const legacyErrorPanic =
    roleKey === "user" &&
    oldResponse.status === 500 &&
    (path.startsWith("/private/satkers") ||
      path === `/private/operasis/${operationId}`);
  if (legacyErrorPanic) {
    skip(
      `${path} [legacy]`,
      "Go server panics in zerolog on Windows without Asia/Makassar tzdata",
    );
    check(`${path} [request-id]`, Boolean(newResponse.requestId));
  } else {
    const oldNormalized = JSON.stringify(normalize(oldResponse.body));
    const newNormalized = JSON.stringify(normalize(newResponse.body));
    check(
      path,
      oldResponse.status === newResponse.status &&
        oldNormalized === newNormalized &&
        oldResponse.contentType === newResponse.contentType,
      `old=${oldResponse.status} new=${newResponse.status}`,
    );
    check(
      `${path} [request-id]`,
      Boolean(oldResponse.requestId) && Boolean(newResponse.requestId),
    );
  }

  const golden = goldenFile(path, roleKey);
  if (golden) {
    const expected = JSON.parse(await readFile(new URL(golden, goldenDir), "utf8"));
    check(
      `${path} [golden]`,
      JSON.stringify(canonical(newResponse.body)) === JSON.stringify(canonical(expected)),
      "new response differs from golden fixture",
    );
  }
}

for (const path of publicPaths) {
  await comparePath(path, null, null);
}

const roles = [
  ["baselineadmin", "parity-admin"],
  ["baselineadminsatker", "parity-admin-satker"],
  ["baselineuser", "parity-user"],
  ["baselineusersatker", "parity-user-satker"],
];

const roleKeys = {
  baselineadmin: "admin",
  baselineadminsatker: "admin_satker",
  baselineuser: "user",
  baselineusersatker: "user_satker",
};

for (const [username, deviceId] of roles) {
  const token = await login(username, deviceId);
  const roleKey = roleKeys[username];
  for (const path of privatePaths) {
    await comparePath(path, token, deviceId, roleKey);
  }
}

const adminToken = await login("baselineadmin", "parity-admin");
for (const path of adminPaths) {
  await comparePath(path, adminToken, "parity-admin", "admin");
}

if (failures > 0) {
  console.error(
    `${failures}/${checks} read parity check(s) failed (${skipped} skipped)`,
  );
  process.exit(1);
}
console.log(`read parity: PASS (${checks} checks, ${skipped} skipped)`);
