#!/usr/bin/env node
// Automated Gate G1 browser stress run.
//
// Serves the Trunk bundle, drives Chrome/Edge through the DevTools Protocol,
// runs the nine marker scenarios (3 counts x 3 modes), reads the result that
// the client publishes in `document.title`, and writes a JSON + Markdown
// report. No npm dependencies: Node 22+ ships a global WebSocket.
//
// Usage:
//   node tools/web-stress.mjs
//   node tools/web-stress.mjs --headless=false --sample=10
//   node tools/web-stress.mjs --only=animated:1000

import { spawn } from "node:child_process";
import { createServer, request as httpRequest } from "node:http";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const distDir = join(projectRoot, "apps", "web", "dist");
const docsDir = join(projectRoot, "docs");
const shotDir = join(docsDir, "assets", "web-stress");
const tileBackend = process.env.TILE_PROXY_BACKEND ?? "http://127.0.0.1:8010";

const COUNTS = [1_000, 5_000, 10_000];
const MODES = [
  { key: "animated", label: "Animated" },
  { key: "static", label: "Static" },
  { key: "mixed", label: "Mixed 50/50" },
];

const args = parseArgs(process.argv.slice(2));
const headless = args.headless !== "false";
const warmupSecs = args.warmup ?? "3";
const sampleSecs = args.sample ?? "10";
const zoom = args.zoom ?? "19";
const layer = args.layer ?? "osm";
const lod = args.lod ?? "1";
const proxyMode = args.proxy ?? (args["proxy-test"] ? "same" : null);
const proxyUrl =
  proxyMode && proxyMode !== "same"
    ? String(proxyMode).replace(/\/+$/, "")
    : null;
const only = args.only ? String(args.only) : null;
const tag = args.tag ? String(args.tag) : "";
const timeoutMs = Number(args.timeout ?? 240_000);

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

async function evaluate(cdp, expression, awaitPromise = false) {
  const result = await cdp.send("Runtime.evaluate", {
    expression,
    returnByValue: true,
    awaitPromise,
  });
  return result.result?.value;
}

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
  console.log("[web-stress]", ...values);
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

      if (pathname.startsWith("/tiles/")) {
        const upstream = new URL(`${pathname}${url.search}`, tileBackend);
        const upstreamRequest = httpRequest(
          upstream,
          {
            method: request.method,
            headers: { ...request.headers, host: upstream.host },
          },
          (upstreamResponse) => {
            response.writeHead(upstreamResponse.statusCode ?? 502, {
              "content-type":
                upstreamResponse.headers["content-type"] ??
                "application/octet-stream",
              "cache-control":
                upstreamResponse.headers["cache-control"] ??
                "public, max-age=60",
              "access-control-allow-origin": "*",
            });
            upstreamResponse.pipe(response);
          },
        );
        upstreamRequest.on("error", () => {
          response.writeHead(502).end("tile proxy error");
        });
        request.pipe(upstreamRequest);
        return;
      }
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

function sleep(ms) {
  return new Promise((resolveSleep) => setTimeout(resolveSleep, ms));
}

async function readTitle(cdp) {
  const result = await cdp.send("Runtime.evaluate", {
    expression: "document.title",
    returnByValue: true,
  });
  return result.result?.value ?? "";
}

async function waitForResult(cdp, scenario, startedAt) {
  const signature = `mode=${scenario.modeLabel} count=${scenario.count}`;
  let lastTitle = "";
  while (Date.now() - startedAt < timeoutMs) {
    try {
      lastTitle = await readTitle(cdp);
    } catch (error) {
      lastTitle = `title read failed: ${error}`;
    }
    if (lastTitle.includes("RESULT") && lastTitle.includes(signature)) {
      return { title: lastTitle };
    }
    if (lastTitle.includes("ERROR=timeout") && lastTitle.includes(signature)) {
      return { title: lastTitle, error: "client reported scenario timeout" };
    }
    await sleep(250);
  }
  return { title: lastTitle, error: `timed out after ${timeoutMs} ms` };
}

function parseResult(title) {
  const match = /RESULT mode=(.+?) count=(\d+) fps=([\d.]+) worst_ms=([\d.]+)/.exec(title);
  if (!match) return null;
  return {
    fps: Number(match[3]),
    worstFrameMs: Number(match[4]),
  };
}

async function captureScreenshot(cdp, path) {
  try {
    const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
    await writeFile(path, Buffer.from(shot.data, "base64"));
    return true;
  } catch (error) {
    log("screenshot failed:", error.message);
    return false;
  }
}

async function waitForTitle(cdp, predicate, timeout = 30_000) {
  const deadline = Date.now() + timeout;
  let title = "";
  while (Date.now() < deadline) {
    title = await readTitle(cdp).catch(() => "");
    if (predicate(title)) return title;
    await sleep(250);
  }
  return title;
}

function zoomFromTitle(title) {
  const match = /zoom=([\d.]+)/.exec(title);
  return match ? Number(match[1]) : null;
}

async function dispatchWheel(cdp, x, y, deltaY) {
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseWheel",
    x,
    y,
    deltaX: 0,
    deltaY,
  });
}

async function runWheelTest(cdp, baseUrl) {
  await cdp.send("Page.navigate", { url: `${baseUrl}?zoom=19` });
  const loaded = await waitForTitle(cdp, (title) => title.includes("zoom="));
  const before = zoomFromTitle(loaded);
  if (before === null) return { status: "FAIL: zoom is not published in title" };

  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseMoved",
    x: 900,
    y: 360,
    button: "none",
  });
  await sleep(300);

  for (let step = 0; step < 3; step += 1) {
    await dispatchWheel(cdp, 900, 360, -120);
    await sleep(150);
  }
  await sleep(700);
  const afterIn = zoomFromTitle(await readTitle(cdp));

  for (let step = 0; step < 6; step += 1) {
    await dispatchWheel(cdp, 900, 360, 120);
    await sleep(150);
  }
  await sleep(700);
  const afterOut = zoomFromTitle(await readTitle(cdp));

  const nearMin = afterOut !== null && afterOut <= 2.01;
  const nearMax = afterIn !== null && afterIn >= 25.99;
  const stepwise =
    afterIn !== null &&
    afterOut !== null &&
    !nearMin &&
    !nearMax &&
    Math.abs(afterIn - before) <= 2.0 &&
    Math.abs(afterOut - before) <= 2.5;
  return {
    status: stepwise ? "PASS" : "FAIL: zoom jumped too far",
    before,
    afterIn,
    afterOut,
  };
}

async function pressKey(cdp, key, code, virtualKeyCode) {
  for (const type of ["keyDown", "keyUp"]) {
    await cdp.send("Input.dispatchKeyEvent", {
      type,
      key,
      code,
      text: type === "keyDown" ? key : undefined,
      windowsVirtualKeyCode: virtualKeyCode,
      nativeVirtualKeyCode: virtualKeyCode,
    });
  }
}

function numberFromTitle(title, key) {
  const match = new RegExp(`${key}=([\\d.-]+)`).exec(title);
  return match ? Number(match[1]) : NaN;
}

async function runFlyTest(cdp, baseUrl, screenshotDir, tagPrefix) {
  await cdp.send("Page.navigate", {
    url: `${baseUrl}?zoom=18&bearing=45`,
  });
  const loaded = await waitForTitle(
    cdp,
    (title) => title.includes("zoom=18.00") && title.includes("bearing=45.0"),
    60_000,
  );
  if (!loaded.includes("zoom=18.00") || !loaded.includes("bearing=45.0")) {
    return { status: "FAIL: initial view not ready", title: loaded };
  }
  const beforeLat = numberFromTitle(loaded, "lat");
  const beforeLon = numberFromTitle(loaded, "lon");

  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseMoved",
    x: 900,
    y: 360,
    button: "none",
  });
  await sleep(200);
  for (let click = 0; click < 2; click += 1) {
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mousePressed",
      x: 900,
      y: 360,
      button: "left",
      buttons: 1,
      clickCount: click + 1,
    });
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mouseReleased",
      x: 900,
      y: 360,
      button: "left",
      buttons: 0,
      clickCount: click + 1,
    });
    await sleep(120);
  }
  await sleep(1400);
  const afterFly = await readTitle(cdp);
  const afterLat = numberFromTitle(afterFly, "lat");
  const afterLon = numberFromTitle(afterFly, "lon");
  const moved =
    Number.isFinite(afterLat) &&
    Number.isFinite(afterLon) &&
    (Math.abs(afterLat - beforeLat) > 1e-5 || Math.abs(afterLon - beforeLon) > 1e-5);

  // Keyboard north is the same flight path as clicking the compass.
  await pressKey(cdp, "n", "KeyN", 78);
  await sleep(1000);
  const afterCompass = await readTitle(cdp);
  const bearing = numberFromTitle(afterCompass, "bearing");
  const compassOk = Number.isFinite(bearing) && Math.min(Math.abs(bearing), Math.abs(bearing - 360)) < 1.0;

  const prefix = tagPrefix ? `${tagPrefix}-` : "";
  await captureScreenshot(cdp, join(screenshotDir, `${prefix}fly-after.png`));
  return {
    status: moved && compassOk ? "PASS" : "FAIL",
    beforeLat,
    beforeLon,
    afterLat,
    afterLon,
    bearing,
    title: afterCompass,
  };
}

async function runLodToggleTest(cdp, baseUrl) {
  await cdp.send("Page.navigate", {
    url: `${baseUrl}?stress=static&count=1000&zoom=19&lod=0`,
  });
  const before = await waitForTitle(
    cdp,
    (title) => title.includes("ready=1000/1000") && title.includes("lod=0"),
    60_000,
  );
  if (!before.includes("ready=1000/1000") || !before.includes("lod=0")) {
    return { status: "FAIL: full-detail scene did not become ready", before };
  }

  await pressKey(cdp, "m", "KeyM", 77);
  const after = await waitForTitle(
    cdp,
    (title) => title.includes("lod=1") && title.includes("ready=1000/1000"),
    90_000,
  );
  return {
    status: after.includes("lod=1") && after.includes("ready=1000/1000") ? "PASS" : "FAIL",
    before,
    after,
  };
}

async function runSelectTest(cdp, baseUrl, screenshotDir, tagPrefix) {
  await cdp.send("Page.navigate", {
    url: `${baseUrl}?stress=static&count=1000&zoom=19&lod=1`,
  });
  const loaded = await waitForTitle(
    cdp,
    (title) => title.includes("ready=1000/1000"),
    60_000,
  );
  if (!loaded.includes("ready=1000/1000")) {
    return { status: "FAIL: scene did not become ready", title: loaded };
  }
  const beforeSel = /sel=([^ ]+)/.exec(loaded)?.[1] ?? null;

  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseMoved",
    x: 640,
    y: 360,
    button: "none",
  });
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: 640,
    y: 360,
    button: "left",
    buttons: 1,
    clickCount: 1,
  });
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseReleased",
    x: 640,
    y: 360,
    button: "left",
    buttons: 0,
    clickCount: 1,
  });

  const after = await waitForTitle(cdp, (title) => /sel=\d+/.test(title), 10_000);
  const match = /sel=(\d+)/.exec(after);
  const prefix = tagPrefix ? `${tagPrefix}-` : "";
  await captureScreenshot(cdp, join(screenshotDir, `${prefix}select-after.png`));
  return {
    status: match ? "PASS" : "FAIL: no marker selected",
    beforeSel,
    afterSel: match ? Number(match[1]) : null,
    title: after,
  };
}

async function runMemoryTest(cdp, baseUrl) {
  await cdp.send("Page.navigate", {
    url: `${baseUrl}?stress=static&count=10000&zoom=19&lod=1&memory=1`,
  });
  const title = await waitForTitle(
    cdp,
    (value) => /loaded_mb=[\d.]+/.test(value) && /cleared_mb=[\d.]+/.test(value),
    120_000,
  );
  const loaded = Number(/loaded_mb=([\d.]+)/.exec(title)?.[1] ?? NaN);
  const cleared = Number(/cleared_mb=([\d.]+)/.exec(title)?.[1] ?? NaN);
  return {
    status:
      Number.isFinite(loaded) && Number.isFinite(cleared)
        ? "PASS"
        : "FAIL: memory probe did not finish",
    loadedMb: loaded,
    clearedMb: cleared,
    title,
  };
}

async function runProxyTest(cdp, baseUrl, screenshotDir, tagPrefix) {
  const effectiveProxy =
    proxyMode === "same" ? baseUrl.replace(/\/+$/, "") : proxyUrl;
  if (!effectiveProxy) {
    return { status: "FAIL: --proxy=<url> is required" };
  }
  await cdp.send("Page.navigate", {
    url: `${baseUrl}?zoom=18&proxy=${encodeURIComponent(effectiveProxy)}`,
  });
  const title = await waitForTitle(cdp, (value) => value.includes("zoom=18"), 60_000);
  if (!title.includes("zoom=18")) {
    return { status: "FAIL: map did not load", title };
  }
  await sleep(3000);
  const prefix = tagPrefix ? `${tagPrefix}-` : "";
  await captureScreenshot(cdp, join(screenshotDir, `${prefix}proxy.png`));
  const result = await evaluate(
    cdp,
    `(async () => {
      const prefix = ${JSON.stringify(`${effectiveProxy}/tiles/`)};
      const proxyUrls = [];
      const samples = [];
      const cacheNames = [];
      let cacheEntries = 0;
      try {
        for (const name of await caches.keys()) {
          cacheNames.push(name);
          if (!name.startsWith("twg-tiles-")) continue;
          const cache = await caches.open(name);
          const requests = await cache.keys();
          cacheEntries += requests.length;
          for (const request of requests) {
            if (samples.length < 5) samples.push(request.url);
            if (request.url.startsWith(prefix)) proxyUrls.push(request.url);
          }
        }
      } catch {}
      return {
        origin: location.origin,
        prefix,
        cacheNames,
        proxyTiles: proxyUrls.length,
        cacheEntries,
        sample: proxyUrls.slice(0, 3),
        samples,
      };
    })()`,
    true,
  );
  const proxyTiles = result?.proxyTiles ?? 0;
  return {
    status:
      proxyTiles > 0 ? "PASS" : "FAIL: no proxy tiles in Cache Storage",
    ...result,
  };
}

async function dragMap(cdp, fromX, fromY, toX, toY, steps = 6) {
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseMoved",
    x: fromX,
    y: fromY,
    button: "none",
  });
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: fromX,
    y: fromY,
    button: "left",
    buttons: 1,
    clickCount: 1,
  });
  for (let step = 1; step <= steps; step += 1) {
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mouseMoved",
      x: fromX + ((toX - fromX) * step) / steps,
      y: fromY + ((toY - fromY) * step) / steps,
      button: "left",
      buttons: 1,
    });
    await sleep(40);
  }
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseReleased",
    x: toX,
    y: toY,
    button: "left",
    buttons: 0,
    clickCount: 1,
  });
}

async function runPanTest(cdp, baseUrl, screenshotDir, tagPrefix) {
  await cdp.send("Page.navigate", {
    url: `${baseUrl}?stress=static&count=1000&zoom=19&bearing=45`,
  });
  const loaded = await waitForTitle(
    cdp,
    (title) => title.includes("ready=1000/1000") && title.includes("bearing=45.0"),
    60_000,
  );
  if (!loaded.includes("bearing=45.0")) {
    return { status: "FAIL: bearing not applied", title: loaded };
  }
  await sleep(1000);
  const beforeLat = numberFromTitle(loaded, "lat");
  const beforeLon = numberFromTitle(loaded, "lon");

  const prefix = tagPrefix ? `${tagPrefix}-` : "";
  await captureScreenshot(cdp, join(screenshotDir, `${prefix}pan-before.png`));

  // Drag down: screen down is north-east at bearing 45, so the camera must
  // move north-east (both latitude and longitude increase).
  await dragMap(cdp, 700, 260, 700, 420);
  await sleep(700);
  const afterDrag = await readTitle(cdp);
  const afterLat = numberFromTitle(afterDrag, "lat");
  const afterLon = numberFromTitle(afterDrag, "lon");
  const forwardOk =
    Number.isFinite(afterLat) &&
    Number.isFinite(afterLon) &&
    afterLat > beforeLat + 1e-6 &&
    afterLon > beforeLon + 1e-6;
  await captureScreenshot(cdp, join(screenshotDir, `${prefix}pan-after.png`));

  // Drag up must reverse the same movement (south-west at bearing 45).
  await dragMap(cdp, 700, 420, 700, 260);
  await sleep(700);
  const returned = await readTitle(cdp);
  const returnLat = numberFromTitle(returned, "lat");
  const returnLon = numberFromTitle(returned, "lon");
  const backwardOk =
    Number.isFinite(returnLat) &&
    Number.isFinite(returnLon) &&
    returnLat < afterLat - 1e-6 &&
    returnLon < afterLon - 1e-6;

  return {
    status: forwardOk && backwardOk ? "PASS" : "FAIL",
    bearing: 45,
    beforeLat,
    beforeLon,
    afterLat,
    afterLon,
    returnLat,
    returnLon,
  };
}

async function collectTileDiagnostics(cdp) {
  await sleep(1500);
  return await evaluate(
    cdp,
    `(async () => {
      const urls = performance
        .getEntriesByType("resource")
        .map((entry) => entry.name)
        .filter((name) => {
          if (
            name.includes("tile.openstreetmap.org") ||
            name.includes("server.arcgisonline.com")
          ) {
            return true;
          }
          try {
            return new URL(name).pathname.startsWith("/tiles/");
          } catch {
            return false;
          }
        });
      const zooms = [];
      const hosts = [];
      for (const url of urls) {
        const parsed = new URL(url);
        if (!hosts.includes(parsed.hostname)) hosts.push(parsed.hostname);
        const match = /\\/(\\d+)\\/\\d+\\/\\d+(?:\\.png)?$/.exec(parsed.pathname);
        if (match && !zooms.includes(match[1])) zooms.push(match[1]);
      }
      let cacheEntries = 0;
      let cacheNames = [];
      try {
        cacheNames = (await caches.keys()).filter((name) => name.startsWith("twg-tiles-"));
        for (const name of cacheNames) {
          const cache = await caches.open(name);
          cacheEntries += (await cache.keys()).length;
        }
      } catch (error) {
        cacheNames = [String(error)];
      }
      return {
        tileRequests: urls.length,
        requestZooms: zooms.sort(),
        hosts: hosts.sort(),
        cacheEntries,
        cacheNames,
        sample: urls.slice(0, 5),
      };
    })()`,
    true,
  );
}

function markdownTable(results) {
  const lines = [
    "| Mode | Count | FPS | Worst frame (ms) | Status |",
    "|---|---:|---:|---:|---|",
  ];
  for (const result of results) {
    lines.push(
      `| ${result.modeLabel} | ${result.count} | ${result.fps?.toFixed(2) ?? "—"} | ${result.worstFrameMs?.toFixed(2) ?? "—"} | ${result.status} |`,
    );
  }
  return lines.join("\n");
}

async function main() {
  if (!existsSync(join(distDir, "index.html"))) {
    throw new Error("apps/web/dist is missing; run `trunk build --cargo-profile wasm-release` first");
  }
  await mkdir(shotDir, { recursive: true });

  const server = await startServer();
  const port = server.address().port;
  const baseUrl = `http://127.0.0.1:${port}/`;
  const debugPort = 9333;
  const userDataDir = join(tmpdir(), `twg-web-stress-${Date.now()}`);
  const chrome = findChrome();
  log(`serving ${distDir} at ${baseUrl}`);
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
  ];
  if (headless) chromeArgs.push("--headless=new");
  chromeArgs.push("about:blank");

  const child = spawn(chrome, chromeArgs, {
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  const chromeLog = [];
  child.stdout.on("data", (data) => chromeLog.push(String(data)));
  child.stderr.on("data", (data) => chromeLog.push(String(data)));

  let cdp = null;
  const results = [];
  try {
    await waitForDebugger(debugPort);
    const target = await createTarget(debugPort, "about:blank");
    cdp = new Cdp(target.webSocketDebuggerUrl);
    await cdp.open();
    await cdp.send("Page.enable");
    await cdp.send("Runtime.enable");
    await cdp.send("Log.enable");
    await cdp.send("Page.bringToFront");
    await cdp.send("Emulation.setDeviceMetricsOverride", {
      width: 1280,
      height: 720,
      deviceScaleFactor: 1,
      mobile: false,
    });

    const consoleLines = [];
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

    await cdp.send("Page.navigate", { url: baseUrl });
    await sleep(1500);
    const renderer = await evaluate(
      cdp,
      `(() => {
        const canvas = document.createElement("canvas");
        const gl = canvas.getContext("webgl2");
        if (!gl) return "no-webgl2";
        const ext = gl.getExtension("WEBGL_debug_renderer_info");
        return ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : "unknown";
      })()`,
    );
    log(`WebGL2 renderer: ${renderer}`);

    const webglVersion = await evaluate(
      cdp,
      `(() => {
        const canvas = document.createElement("canvas");
        const gl = canvas.getContext("webgl2");
        return gl ? gl.getParameter(gl.VERSION) : "no-webgl2";
      })()`,
    );
    log(`WebGL2 version: ${webglVersion}`);

    for (const mode of MODES) {
      for (const count of COUNTS) {
        if (only && only !== `${mode.key}:${count}`) continue;
        const scenario = { ...mode, modeLabel: mode.label, count };
        const params = new URLSearchParams({
          stress: mode.key,
          count: String(count),
          benchmark: "1",
          warmup: String(warmupSecs),
          sample: String(sampleSecs),
          zoom: String(zoom),
          layer: String(layer),
          lod: String(lod),
        });
        const url = `${baseUrl}?${params.toString()}`;
        log(`running ${mode.label} x${count}`);
        const startedAt = Date.now();
        await cdp.send("Page.navigate", { url });
        const outcome = await waitForResult(cdp, scenario, startedAt);
        const parsed = parseResult(outcome.title);
        const result = {
          mode: mode.key,
          modeLabel: mode.label,
          count,
          fps: parsed?.fps ?? null,
          worstFrameMs: parsed?.worstFrameMs ?? null,
          status: outcome.error ?? "PASS",
          title: outcome.title,
          seconds: Number(((Date.now() - startedAt) / 1000).toFixed(1)),
        };
        results.push(result);
        if (parsed) {
          log(`  ${parsed.fps.toFixed(2)} fps, worst ${parsed.worstFrameMs.toFixed(2)} ms`);
        } else {
          log(`  FAILED: ${outcome.error ?? "no result in title"} (title: ${outcome.title})`);
        }
        await sleep(500);
        await captureScreenshot(
          cdp,
          join(shotDir, `${tag ? `${tag}-` : ""}${mode.key}-${count}.png`),
        );
      }
    }

    const diagnostics = await collectTileDiagnostics(cdp);
    log(
      `tile requests: ${diagnostics.tileRequests}, zooms ${JSON.stringify(diagnostics.requestZooms)}, cache ${diagnostics.cacheEntries}`,
    );

    let inputDiagnostics = null;
    if (
      args["wheel-test"] ||
      args["pan-test"] ||
      args["lod-toggle-test"] ||
      args["select-test"] ||
      args["memory-test"] ||
      args["proxy-test"] ||
      args["fly-test"]
    ) {
      inputDiagnostics = {};
      if (args["wheel-test"]) {
        log("running wheel zoom test");
        inputDiagnostics.wheel = await runWheelTest(cdp, baseUrl);
        log(`wheel test: ${JSON.stringify(inputDiagnostics.wheel)}`);
      }
      if (args["pan-test"]) {
        log("running pan anchoring test");
        inputDiagnostics.pan = await runPanTest(cdp, baseUrl, shotDir, tag);
        log(`pan test: ${JSON.stringify(inputDiagnostics.pan)}`);
      }
      if (args["lod-toggle-test"]) {
        log("running runtime LOD toggle test");
        inputDiagnostics.lodToggle = await runLodToggleTest(cdp, baseUrl);
        log(`lod toggle test: ${JSON.stringify(inputDiagnostics.lodToggle)}`);
      }
      if (args["select-test"]) {
        log("running marker selection test");
        inputDiagnostics.select = await runSelectTest(cdp, baseUrl, shotDir, tag);
        log(`select test: ${JSON.stringify(inputDiagnostics.select)}`);
      }
      if (args["memory-test"]) {
        log("running memory load/unload test");
        inputDiagnostics.memory = await runMemoryTest(cdp, baseUrl);
        log(`memory test: ${JSON.stringify(inputDiagnostics.memory)}`);
      }
      if (args["proxy-test"]) {
        log("running tile proxy test");
        inputDiagnostics.proxy = await runProxyTest(cdp, baseUrl, shotDir, tag);
        log(`proxy test: ${JSON.stringify(inputDiagnostics.proxy)}`);
      }
      if (args["fly-test"]) {
        log("running fly-to and compass test");
        inputDiagnostics.fly = await runFlyTest(cdp, baseUrl, shotDir, tag);
        log(`fly test: ${JSON.stringify(inputDiagnostics.fly)}`);
      }
    }

    let switchDiagnostics = null;
    if (args["switch-layer"]) {
      log("switching base layer with L");
      await cdp.send("Page.bringToFront");
      for (const type of ["keyDown", "keyUp"]) {
        await cdp.send("Input.dispatchKeyEvent", {
          type,
          key: "l",
          code: "KeyL",
          text: type === "keyDown" ? "l" : undefined,
          windowsVirtualKeyCode: 76,
          nativeVirtualKeyCode: 76,
        });
      }
      await sleep(4000);
      await captureScreenshot(
        cdp,
        join(shotDir, `${tag ? `${tag}-` : ""}after-switch.png`),
      );
      switchDiagnostics = await collectTileDiagnostics(cdp);
      log(
        `after switch: hosts ${JSON.stringify(switchDiagnostics.hosts)}, cache ${switchDiagnostics.cacheEntries}`,
      );
    }

    const suffix = tag ? `-${tag}` : "";
    await writeFile(
      join(docsDir, `web-stress-console${suffix}.log`),
      `${consoleLines.join("\n")}\n`,
    );
    await writeFile(
      join(docsDir, `web-stress-results${suffix}.json`),
      `${JSON.stringify(
        {
          generatedAt: new Date().toISOString(),
          renderer,
          webglVersion,
          headless,
          warmupSecs: Number(warmupSecs),
          sampleSecs: Number(sampleSecs),
          zoom: Number(zoom),
          layer,
          lod: Number(lod) === 1,
          diagnostics,
          inputDiagnostics,
          switchedDiagnostics: switchDiagnostics,
          results,
        },
        null,
        2,
      )}\n`,
    );
    const table = markdownTable(results);
    await writeFile(
      join(docsDir, `web-stress-results${suffix}.md`),
      `# Hasil stress test web — Gate G1\n\n` +
        `Renderer: ${renderer} (${webglVersion})\n\n` +
        `Warmup ${warmupSecs}s, sample ${sampleSecs}s, zoom ${zoom}, layer ${layer}, LOD ${lod}.\n\n` +
        `${table}\n\n` +
        `Tile requests: ${diagnostics.tileRequests}; request zooms: ${diagnostics.requestZooms.join(", ") || "—"}; Cache Storage entries: ${diagnostics.cacheEntries}.\n`,
    );
    console.log(`\n${table}\n`);
  } finally {
    cdp?.close();
    child.kill();
    await sleep(500);
    server.close();
    try {
      await rm(userDataDir, { recursive: true, force: true });
    } catch {
      // temporary profile can stay if the browser still holds a file
    }
  }
}

main().catch((error) => {
  console.error("[web-stress] fatal:", error);
  process.exitCode = 1;
});
