// Persistent XYZ tile cache for the web client.
//
// The Bevy asset loader fetches raster tiles directly from the provider.
// A service worker is the only place that can cache those cross-origin
// responses across reloads without a backend proxy, so viewed tiles survive
// navigation, panning back, and restarts. Only provider hosts below are
// intercepted; application assets keep using the normal HTTP cache.
//
// The client already clamps tile requests to the provider's native max zoom
// (19 for OSM and Esri World Imagery), so overzoom at camera z20..z26 reuses
// the cached z19 tile instead of requesting a nonexistent level.

const CACHE_PREFIX = "twg-tiles-";
const CACHE_NAME = `${CACHE_PREFIX}v1`;
const MAX_ENTRIES = 1500;
const TILE_HOSTS = new Set([
  "tile.openstreetmap.org",
  "a.tile.openstreetmap.org",
  "b.tile.openstreetmap.org",
  "c.tile.openstreetmap.org",
  "server.arcgisonline.com",
]);

self.addEventListener("install", (event) => {
  event.waitUntil(self.skipWaiting());
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      const names = await caches.keys();
      await Promise.all(
        names
          .filter((name) => name.startsWith(CACHE_PREFIX) && name !== CACHE_NAME)
          .map((name) => caches.delete(name)),
      );
      await self.clients.claim();
    })(),
  );
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;
  let url;
  try {
    url = new URL(request.url);
  } catch {
    return;
  }
  const isTile =
    TILE_HOSTS.has(url.hostname) || url.pathname.startsWith("/tiles/");
  if (!isTile) return;
  event.respondWith(cacheFirst(event, request));
});

async function cacheFirst(event, request) {
  const cache = await caches.open(CACHE_NAME);
  // `ignoreVary` is required because both providers vary the response on
  // Origin and the cached request may come from a different client origin.
  const cached = await cache.match(request, { ignoreVary: true });
  if (cached) {
    return cached;
  }

  const response = await fetch(request);
  if (response.ok || response.type === "opaque") {
    event.waitUntil(store(cache, request, response.clone()));
  }
  return response;
}

async function store(cache, request, response) {
  try {
    await cache.put(request, response);
    const keys = await cache.keys();
    if (keys.length > MAX_ENTRIES) {
      await Promise.all(
        keys.slice(0, keys.length - MAX_ENTRIES).map((key) => cache.delete(key)),
      );
    }
  } catch (error) {
    // Quota errors must never break tile rendering.
    console.warn("tile cache write failed", error);
  }
}
