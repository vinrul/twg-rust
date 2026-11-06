#!/usr/bin/env node
// Client-shell smoke test for the Rust Bevy web build.
//
// Serves `apps/web/dist`, restores a real session from the API into
// `localStorage`, boots the app in headless Chrome, and verifies that the
// authenticated screens issue their API requests without JS exceptions.
// Also writes a screenshot for manual review.
//
// Usage:
//   trunk build
//   node tools/client-smoke.mjs
//   node tools/client-smoke.mjs --headless=false --api=http://127.0.0.1:8010

import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { existsSync } from "node:fs";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const distDir = join(projectRoot, "apps", "web", "dist");
const shotDir = join(projectRoot, "docs", "assets");

const args = parseArgs(process.argv.slice(2));
const apiBase = (args.api ?? process.env.TWG_API_URL ?? "http://127.0.0.1:8010").replace(/\/+$/, "");
const username = args.username ?? process.env.TWG_USER ?? "baselineadmin";
const password = args.password ?? process.env.TWG_PASSWORD ?? "Baseline#2026";
const headless = args.headless !== "false";
const timeoutMs = Number(args.timeout ?? 120_000);
const shotName = args.shot ?? "phase8-client.png";
const previewShot = args["preview-shot"] ?? "phase8-client-map.png";
const testPreview = args.preview !== "false";
const viewZoom = args.zoom ?? null;
const editorScenario = args.scenario ?? null;
const editorTest = args["editor-test"] === "true";
const mapWaitMs = Number(args["map-wait"] ?? 15000);

const MIME = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".mjs", "text/javascript; charset=utf-8"],
  [".css", "text/css; charset=utf-8"],
  [".wasm", "application/wasm"],
  [".json", "application/json"],
  [".png", "image/png"],
  [".jpg", "image/jpeg"],
  [".jpeg", "image/jpeg"],
  [".webp", "image/webp"],
  [".glb", "model/gltf-binary"],
  [".gltf", "model/gltf+json"],
  [".bin", "application/octet-stream"],
  [".svg", "image/svg+xml"],
]);

function parseArgs(argv) {
  const parsed = {};
  for (const item of argv) {
    if (!item.startsWith("--")) continue;
    const [key, value = "true"] = item.slice(2).split("=");
    parsed[key] = value;
  }
  return parsed;
}

function log(...values) {
  console.log("[client-smoke]", ...values);
}

function sleep(ms) {
    return new Promise((resolveSleep) => setTimeout(resolveSleep, ms));
}

async function clickAt(cdp, x, y) {
    for (const type of ["mousePressed", "mouseReleased"]) {
        await cdp.send("Input.dispatchMouseEvent", {
            type,
            x,
            y,
            button: "left",
            clickCount: 1,
        });
    }
}

async function pressKey(cdp, key, code, vk, modifiers = 0) {
    await cdp.send("Input.dispatchKeyEvent", {
        type: "keyDown",
        key,
        code,
        text: modifiers === 0 && key.length === 1 ? key : undefined,
        windowsVirtualKeyCode: vk,
        nativeVirtualKeyCode: vk,
        modifiers,
    });
    await cdp.send("Input.dispatchKeyEvent", {
        type: "keyUp",
        key,
        code,
        windowsVirtualKeyCode: vk,
        nativeVirtualKeyCode: vk,
        modifiers,
    });
}

async function pressCtrl(cdp, key, code, vk) {
    await cdp.send("Input.dispatchKeyEvent", {
        type: "keyDown",
        key: "Control",
        code: "ControlLeft",
        windowsVirtualKeyCode: 17,
        nativeVirtualKeyCode: 17,
        modifiers: 2,
    });
    await pressKey(cdp, key, code, vk, 2);
    await cdp.send("Input.dispatchKeyEvent", {
        type: "keyUp",
        key: "Control",
        code: "ControlLeft",
        windowsVirtualKeyCode: 17,
        nativeVirtualKeyCode: 17,
    });
}

function findChrome() {
  const candidates = [
    process.env.CHROME_PATH,
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
    "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
  ].filter(Boolean);
  for (const candidate of candidates) {
    if (existsSync(candidate)) return candidate;
  }
  throw new Error("Chrome/Edge not found; set CHROME_PATH");
}

async function startServer() {
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url ?? "/", "http://127.0.0.1");
      let pathname = decodeURIComponent(url.pathname);
      if (pathname.endsWith("/")) pathname += "index.html";
      const relative = normalize(pathname).replace(/^([/\\])+/, "");
      const filePath = join(distDir, relative);
      if (!filePath.startsWith(distDir)) {
        response.writeHead(403).end("forbidden");
        return;
      }
      const data = await readFile(filePath);
      response.writeHead(200, {
        "content-type": MIME.get(extname(filePath).toLowerCase()) ?? "application/octet-stream",
        "cache-control": "no-cache",
        "access-control-allow-origin": "*",
      });
      response.end(data);
    } catch {
      response.writeHead(404).end("not found");
    }
  });
  await new Promise((resolveListen, rejectListen) => {
    server.once("error", rejectListen);
    server.listen(0, "127.0.0.1", resolveListen);
  });
  return server;
}

async function waitForDebugger(port) {
  const deadline = Date.now() + 30_000;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (response.ok) return await response.json();
    } catch (error) {
      lastError = error;
    }
    await sleep(200);
  }
  throw new Error(`debugger did not start: ${lastError}`);
}

async function createTarget(port, url) {
  const response = await fetch(
    `http://127.0.0.1:${port}/json/new?${encodeURIComponent(url)}`,
    { method: "PUT" },
  );
  if (!response.ok) {
    throw new Error(`could not create target: ${response.status}`);
  }
  return await response.json();
}

class Cdp {
  constructor(webSocketUrl) {
    this.nextId = 1;
    this.pending = new Map();
    this.listeners = new Map();
    this.socket = new WebSocket(webSocketUrl);
  }

  async open() {
    await new Promise((resolveOpen, rejectOpen) => {
      this.socket.addEventListener("open", resolveOpen, { once: true });
      this.socket.addEventListener("error", rejectOpen, { once: true });
    });
    this.socket.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      if (message.id) {
        const pending = this.pending.get(message.id);
        if (!pending) return;
        this.pending.delete(message.id);
        if (message.error) pending.reject(new Error(JSON.stringify(message.error)));
        else pending.resolve(message.result);
        return;
      }
      const handlers = this.listeners.get(message.method);
      if (handlers) {
        for (const handler of handlers) handler(message.params);
      }
    });
  }

  send(method, params = {}) {
    const id = this.nextId++;
    return new Promise((resolveSend, rejectSend) => {
      this.pending.set(id, { resolve: resolveSend, reject: rejectSend });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  on(method, handler) {
    const handlers = this.listeners.get(method) ?? [];
    handlers.push(handler);
    this.listeners.set(method, handlers);
  }

  close() {
    try {
      this.socket.close();
    } catch {
      // already closed
    }
  }
}

const editorHeaders = (token) => ({
    authorization: `Bearer ${token}`,
    "x-device-id": "client-smoke-web",
  });

async function countPath(token, path) {
    if (!editorScenario) return -1;
    const response = await fetch(
      `${apiBase}/public/${path}/list/${encodeURIComponent(editorScenario)}`,
      { headers: editorHeaders(token) },
    );
    if (!response.ok) return -1;
    const body = await response.json();
    return Array.isArray(body.data) ? body.data.length : -1;
}

async function countMarkers(token) {
    return countPath(token, "markers");
}

async function listItems(token, path) {
    if (!editorScenario) return [];
    const response = await fetch(
      `${apiBase}/public/${path}/list/${encodeURIComponent(editorScenario)}`,
      { headers: editorHeaders(token) },
    );
    if (!response.ok) return [];
    const body = await response.json();
    return body.data ?? [];
}

async function deleteNewIds(token, path, baselineIds) {
    let deleted = 0;
    for (const item of await listItems(token, path)) {
      if (baselineIds.has(item.id)) continue;
      const result = await fetch(`${apiBase}/private/${path}/${item.id}`, {
        method: "DELETE",
        headers: editorHeaders(token),
      });
      if (result.ok) deleted += 1;
    }
    return deleted;
}

async function login() {
  const response = await fetch(`${apiBase}/auth/login`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      username,
      password,
      device_id: "client-smoke-web",
    }),
  });
  if (!response.ok) {
    throw new Error(`login failed: ${response.status} ${await response.text()}`);
  }
  const body = await response.json();
  return body.data;
}

async function main() {
  if (!existsSync(join(distDir, "index.html"))) {
    throw new Error("apps/web/dist is missing; run `trunk build` first");
  }
  await mkdir(shotDir, { recursive: true });

  log(`logging in against ${apiBase} as ${username}`);
  const session = await login();
  const stored = {
    access_token: session.access_token,
    refresh_token: session.refresh_token,
    access_expires_at: session.access_token_expires_at,
    refresh_expires_at: session.refresh_token_expires_at,
    device_id: "client-smoke-web",
    user: session.user,
  };

  const server = await startServer();
  const port = server.address().port;
  const pageUrl = `http://127.0.0.1:${port}/?api=${encodeURIComponent(apiBase)}${
    viewZoom ? `&zoom=${encodeURIComponent(viewZoom)}` : ""
  }${editorScenario ? `&scenario=${encodeURIComponent(editorScenario)}` : ""}`;
  const debugPort = 9334;
  const userDataDir = join(tmpdir(), `twg-client-smoke-${Date.now()}`);
  const chrome = findChrome();
  log(`serving ${distDir} at http://127.0.0.1:${port}`);
  log(`launching ${chrome}`);

  const chromeArgs = [
    `--remote-debugging-port=${debugPort}`,
    `--user-data-dir=${userDataDir}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-renderer-backgrounding",
    "--disable-features=CalculateNativeWinOcclusion",
    "--window-size=1280,720",
    "--window-position=0,0",
    "--autoplay-policy=no-user-gesture-required",
    "--use-angle=swiftshader",
  ];
  if (headless) chromeArgs.push("--headless=new");
  chromeArgs.push("about:blank");

  const child = spawn(chrome, chromeArgs, {
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  child.stdout.on("data", () => {});
  child.stderr.on("data", () => {});

  let cdp = null;
  const consoleLines = [];
  const responses = [];
  const requests = [];
  const webSockets = [];
  try {
    await waitForDebugger(debugPort);
    const target = await createTarget(debugPort, "about:blank");
    cdp = new Cdp(target.webSocketDebuggerUrl);
    await cdp.open();
    await cdp.send("Page.enable");
    await cdp.send("Runtime.enable");
    await cdp.send("Log.enable");
    await cdp.send("Network.enable");
    await cdp.send("Page.bringToFront");
    await cdp.send("Emulation.setDeviceMetricsOverride", {
      width: 1280,
      height: 720,
      deviceScaleFactor: 1,
      mobile: false,
    });

    cdp.on("Runtime.consoleAPICalled", (event) => {
      const message = (event.args ?? [])
        .map((arg) => arg.value ?? arg.description ?? arg.type)
        .join(" ");
      if (message) consoleLines.push(`${event.type}: ${message}`);
    });
    cdp.on("Runtime.exceptionThrown", (event) => {
      consoleLines.push(`exception: ${event.exceptionDetails?.text ?? "unknown"}`);
    });
    cdp.on("Log.entryAdded", (event) => {
      consoleLines.push(`${event.entry.level}: ${event.entry.text}`);
    });
    cdp.on("Network.requestWillBeSent", (event) => {
      requests.push({ url: event.request.url, method: event.request.method });
    });
    cdp.on("Network.responseReceived", (event) => {
      responses.push({
        url: event.response.url,
        status: event.response.status,
        mime: event.response.mimeType,
      });
    });
    cdp.on("Network.webSocketCreated", (event) => {
      webSockets.push(event.url);
    });

    await cdp.send("Page.addScriptToEvaluateOnNewDocument", {
      source: `try {
        localStorage.setItem("twg.session", ${JSON.stringify(JSON.stringify(stored))});
        localStorage.setItem("twg.device_id", "client-smoke-web");
      } catch (error) {
        console.error("session seed failed", error);
      }`,
    });
    log(`navigating to ${pageUrl}`);
    await cdp.send("Page.navigate", { url: pageUrl });

    const deadline = Date.now() + timeoutMs;
    let identified = false;
    let dashboard = false;
    while (Date.now() < deadline) {
      identified ||= responses.some(
        (item) => item.url.endsWith("/private/identify") && item.status === 200,
      );
      dashboard ||= responses.some(
        (item) => item.url.endsWith("/public/list/operasis") && item.status === 200,
      );
      if (identified && dashboard) break;
      await sleep(500);
    }

    await sleep(2500);
    const screenshot = await cdp.send("Page.captureScreenshot", { format: "png" });
    await writeFile(join(shotDir, shotName), Buffer.from(screenshot.data, "base64"));

    let wsTicket = false;
    if (testPreview) {
      // Sidebar item "Peta" at the fixed 1280x720 smoke layout.
      await cdp.send("Input.dispatchMouseEvent", {
        type: "mousePressed",
        x: 60,
        y: 436,
        button: "left",
        clickCount: 1,
      });
      await cdp.send("Input.dispatchMouseEvent", {
        type: "mouseReleased",
        x: 60,
        y: 436,
        button: "left",
        clickCount: 1,
      });
      const wsDeadline = Date.now() + 30_000;
      while (Date.now() < wsDeadline) {
        wsTicket ||= responses.some(
          (item) => item.url.endsWith("/private/ws-ticket") && item.status === 200,
        );
        if (wsTicket && webSockets.length > 0) break;
        await sleep(500);
      }
      await sleep(mapWaitMs);
      const mapScreenshot = await cdp.send("Page.captureScreenshot", { format: "png" });
      await writeFile(
        join(shotDir, previewShot),
        Buffer.from(mapScreenshot.data, "base64"),
      );
    }

    let editorFailures = [];
    if (editorTest) {
      await sleep(4000);
      const baselineMarkers = await countMarkers(session.access_token);
      const baselineLines = await countPath(session.access_token, "lines");
      const baselineBuildings = await countPath(session.access_token, "buildings");
      const baselineTexts = await countPath(session.access_token, "text-markers");
      const baselineIds = {
        markers: new Set(
          (await listItems(session.access_token, "markers")).map((item) => item.id),
        ),
        lines: new Set(
          (await listItems(session.access_token, "lines")).map((item) => item.id),
        ),
        buildings: new Set(
          (await listItems(session.access_token, "buildings")).map((item) => item.id),
        ),
        texts: new Set(
          (await listItems(session.access_token, "text-markers")).map((item) => item.id),
        ),
      };
      // Scene row "Unit Alpha 1" in the editor hierarchy at 1280x720.
      await clickAt(cdp, 120, 432);
      await sleep(900);
      await pressKey(cdp, "w", "KeyW", 87);
      await sleep(1500);
      const selectShot = await cdp.send("Page.captureScreenshot", { format: "png" });
      await writeFile(
        join(shotDir, "phase9-editor-select.png"),
        Buffer.from(selectShot.data, "base64"),
      );

      // Toolbar: P Marker, then click the viewport, then Simpan.
      await clickAt(cdp, 398, 53);
      await sleep(1200);
      await clickAt(cdp, 600, 300);
      await sleep(1500);

      // Line: two points, then a quick double-click to finish.
      await clickAt(cdp, 483, 53);
      await sleep(1800);
      await clickAt(cdp, 470, 330);
      await sleep(1800);
      await clickAt(cdp, 560, 340);
      await sleep(1800);
      await pressKey(cdp, "Enter", "Enter", 13);
      await sleep(1800);

      // Polygon/building: three points and a double-click.
      await clickAt(cdp, 573, 53);
      await sleep(1800);
      await clickAt(cdp, 700, 320);
      await sleep(1800);
      await clickAt(cdp, 760, 330);
      await sleep(1800);
      await clickAt(cdp, 730, 390);
      await sleep(1800);
      await pressKey(cdp, "Enter", "Enter", 13);
      await sleep(1800);

      // Text marker.
      await clickAt(cdp, 663, 53);
      await sleep(1800);
      await clickAt(cdp, 620, 380);
      await sleep(1800);

      await clickAt(cdp, 1082, 53);
      await sleep(7000);
      const createShot = await cdp.send("Page.captureScreenshot", { format: "png" });
      await writeFile(
        join(shotDir, "phase9-editor-create.png"),
        Buffer.from(createShot.data, "base64"),
      );
      const created = responses.some(
        (item) => item.url.endsWith("/private/markers") && item.status === 201,
      );
      if (!created) editorFailures.push("editor did not POST /private/markers");

      const baseline = Number.isInteger(baselineMarkers) ? baselineMarkers : 0;
      const countAfterCreate = await countMarkers(session.access_token);
      if (countAfterCreate !== baseline + 1) {
        editorFailures.push(
          `expected ${baseline + 1} markers after save, got ${countAfterCreate}`,
        );
      }
      const linesAfterCreate = await countPath(session.access_token, "lines");
      if (linesAfterCreate !== baselineLines + 1) {
        editorFailures.push(
          `expected ${baselineLines + 1} lines after save, got ${linesAfterCreate}`,
        );
      }
      const buildingsAfterCreate = await countPath(session.access_token, "buildings");
      if (buildingsAfterCreate !== baselineBuildings + 1) {
        editorFailures.push(
          `expected ${baselineBuildings + 1} buildings after save, got ${buildingsAfterCreate}`,
        );
      }
      const textsAfterCreate = await countPath(session.access_token, "text-markers");
      if (textsAfterCreate !== baselineTexts + 1) {
        editorFailures.push(
          `expected ${baselineTexts + 1} texts after save, got ${textsAfterCreate}`,
        );
      }

      await cdp.send("Page.reload");
      await sleep(16000);
      const reloadShot = await cdp.send("Page.captureScreenshot", { format: "png" });
      await writeFile(
        join(shotDir, "phase9-editor-reload.png"),
        Buffer.from(reloadShot.data, "base64"),
      );
      const countAfterReload = await countMarkers(session.access_token);
      if (countAfterReload !== baseline + 1) {
        editorFailures.push(
          `expected ${baseline + 1} markers after reload, got ${countAfterReload}`,
        );
      }
      const linesAfterReload = await countPath(session.access_token, "lines");
      const buildingsAfterReload = await countPath(session.access_token, "buildings");
      const textsAfterReload = await countPath(session.access_token, "text-markers");
      if (
        linesAfterReload !== baselineLines + 1 ||
        buildingsAfterReload !== baselineBuildings + 1 ||
        textsAfterReload !== baselineTexts + 1
      ) {
        editorFailures.push(
          `reload counts lines=${linesAfterReload} buildings=${buildingsAfterReload} texts=${textsAfterReload}`,
        );
      }
      const removedMarkers = await deleteNewIds(
        session.access_token,
        "markers",
        baselineIds.markers,
      );
      const removedLines = await deleteNewIds(
        session.access_token,
        "lines",
        baselineIds.lines,
      );
      const removedBuildings = await deleteNewIds(
        session.access_token,
        "buildings",
        baselineIds.buildings,
      );
      const removedTexts = await deleteNewIds(
        session.access_token,
        "text-markers",
        baselineIds.texts,
      );
      if (removedLines !== 1 || removedBuildings !== 1 || removedTexts !== 1) {
        editorFailures.push(
          `cleanup removed lines=${removedLines} buildings=${removedBuildings} texts=${removedTexts}`,
        );
      }
      const finalMarkers = await countMarkers(session.access_token);
      if (removedMarkers > 0) {
        log(`cleanup removed ${removedMarkers} leftover marker(s)`);
      }
      if (finalMarkers !== baseline) {
        editorFailures.push(`expected ${baseline} markers after cleanup, got ${finalMarkers}`);
      }

      // Select the newly created marker and delete it through the inspector.
      // The API orders markers by created_at DESC, so the new one is first.
      await clickAt(cdp, 120, 432);
      await sleep(1500);
      await clickAt(cdp, 1220, 141);
      await sleep(1200);
      await clickAt(cdp, 1082, 53);
      await sleep(6000);
      const countAfterDelete = await countMarkers(session.access_token);
      if (countAfterDelete !== baseline) {
        editorFailures.push(
          `expected ${baseline} markers after delete, got ${countAfterDelete}`,
        );
      }
      const deleted = responses.some(
        (item) =>
          item.url.includes("/private/markers/") && item.status === 200,
      );
      if (!deleted) editorFailures.push("editor did not DELETE /private/markers/{id}");
    }

    const title = await cdp.send("Runtime.evaluate", {
      expression: "document.title",
      returnByValue: true,
    });
    const storedCheck = await cdp.send("Runtime.evaluate", {
      expression: "localStorage.getItem('twg.session') !== null",
      returnByValue: true,
    });

    const failures = [];
    if (!identified) failures.push("missing 200 /private/identify");
    if (!dashboard) failures.push("missing 200 /public/list/operasis");
    if (storedCheck.result?.value !== true) failures.push("session not in localStorage");
    if (!title.result?.value) failures.push("document title is empty");
    const exceptions = consoleLines.filter((line) => line.startsWith("exception:"));
    if (exceptions.length > 0) failures.push(`${exceptions.length} JS exception(s)`);

    if (testPreview) {
      if (!wsTicket) failures.push("missing 200 /private/ws-ticket");
      if (webSockets.length === 0) failures.push("no WebSocket connection");
    }
    failures.push(...editorFailures);
    log(`title: ${title.result?.value}`);
    log(`API calls: ${responses.filter((item) => item.url.includes("/auth/") || item.url.includes("/private/") || item.url.includes("/public/")).length}`);
    if (testPreview) log(`websockets: ${webSockets.join(", ")}`);
    log(
      `tile requests: ${requests.filter((item) => /tile|arcgisonline/i.test(item.url)).length}`,
    );
    const tileStatuses = new Map();
    for (const item of responses.filter((response) => /tile|arcgisonline/i.test(response.url))) {
      tileStatuses.set(item.status, (tileStatuses.get(item.status) ?? 0) + 1);
    }
    log(
      `tile responses: ${[...tileStatuses].map(([status, count]) => `${status}x${count}`).join(" ")}`,
    );
    log(`screenshot: ${join(shotDir, shotName)}`);
    if (failures.length > 0) {
      log("FAILED:");
      for (const failure of failures) log(`- ${failure}`);
      for (const line of consoleLines.slice(-20)) log(`  console ${line}`);
      process.exitCode = 1;
      return;
    }
    log("OK");
  } finally {
    cdp?.close();
    child.kill();
    server.close();
    await rm(userDataDir, { recursive: true, force: true }).catch(() => {});
  }
}

main().catch((error) => {
  console.error("[client-smoke] error:", error);
  process.exitCode = 1;
});
