# Laporan Fase 3 — Spike `twg-map` dan Gate G1

**Tanggal:** 5 November 2026

**Status:** native PASS; runtime browser PASS di Chrome WebGL2 untuk 9/9
skenario stress marker, termasuk 10.000 marker beranimasi pada 50,60 FPS.
Minimum seluruh skenario web adalah 47,19 FPS.

**Keputusan:** lanjutkan Bevy + `twg-map`. Gate G1 lulus penuh di
Chrome/Chromium WebGL2: peta, FPS, overzoom, presisi, object picking/selection,
dan pengukuran memori. Fly-to, panel kompas, quadtree tile cover, error tile,
tile proxy server-side, serta overlay line/polygon/floor plan sudah selesai.
Firefox, Safari, dan GPU integrated belum diukur. Laporan rinci ada di
[`WEB_STRESS_REPORT.md`](WEB_STRESS_REPORT.md).

![Native OSM raster, marker GLTF, dan FPS overlay](assets/phase3-native.png)

## Hasil yang sudah dibuktikan

| Area | Hasil |
|---|---|
| Core geospasial | `GeoPoint`, WGS84 → Web Mercator, tile fraction, `TileId`, parent/children, local offset |
| Tile source | XYZ OSM tanpa API key, attribution, URL substitution |
| Tile cover | Quadtree adaptif berdasarkan frustum kamera, maks 15×15, wrap longitude, dedup di zoom rendah |
| Kamera | center, zoom, pitch, bearing; pan, orbit, pitch, wheel zoom, keyboard, reset |
| Fly-to | Animasi 0,8 s dengan easing, wrap longitude, dan jalur bearing terpendek; double-click marker/peta, `N` ke utara |
| Kompas/zoom | Panel kanan atas: needle, N/E/S/W, tombol −, N, +, dan Reset |
| Renderer | Bevy 0.20 + `bevy_egui` 0.43, tile raster, fallback ground, marker GLTF |
| Tile loading | Bevy `https` asset source; native `ureq`, web `fetch`; native disk cache `.web-asset-cache` |
| Error tile | Checkerboard 64×64 untuk tile gagal/missing, diganti saat tile asli tiba |
| Marker | GLB non-Draco dengan dua clip animasi dan embedded JPEG valid |
| Draco | `KHR_draco_mesh_compression` belum didukung Bevy 0.20; probe `*_draco.glb` FAIL, stress memakai non-Draco/LOD |
| Cursor | ray → bidang peta → latitude/longitude di UI |
| Picking | Klik marker → screen-space nearest pick + selection ring 3 m; index tampil di panel |
| Memori native | 10k static: full 491,9 → 500,8 MB, LOD 489,7 → 498,5 MB; animated: full 493,5 → 501,0 MB, LOD 499,7 → 503,6 MB |
| Memori web | WASM linear memory 135,8 MB setelah load dan setelah clear |
| FPS native | 10k animated: 45,69 FPS model penuh, 146,59 FPS LOD; 10k static: 54,29 FPS penuh, 195,39 FPS LOD |
| FPS web | 47,19–100 FPS; minimum 47,19 pada 10.000 static, 50,60 pada 10.000 animated |
| Input web | Wheel stepwise (19 → 19,45 → 18,55), pan mengikuti bearing (45° diuji maju/mundur), toggle LOD runtime, select marker, fly-to PASS |
| Stress web | 9/9 skenario PASS: 1k/5k/10k × animated/static/mixed |
| LOD | `polwan_airud_*_lod2.glb` 2,4k triangle, skeleton + animasi utuh |
| Overzoom | kamera sampai zoom 26, semua tile request tetap z19 |
| Base layer | OSM dan Esri World Imagery, switch runtime, attribution tampil |
| Tile cache | service worker Cache Storage di web; disk cache native; LRU material/image |
| Tile proxy | `GET /tiles/{layer}/{z}/{x}/{y}.png` di Axum; disk cache `.tile-cache/`, semaphore konkurensi, throttle interval, CORS |
| Overlay | Line ribbon, polygon/building extrusion 14 m, floor plan quad + tekstur programatik |
| Test core | 32 test `twg-map` + 5 test `twg-render` + 5 test `twg-server` PASS |
| Dependency policy | `cargo deny check` PASS; `CDLA-Permissive-2.0` ditambahkan untuk `webpki-root-certs` |
| Native | desktop berjalan tanpa panic; tile OSM, marker, dan stress benchmark tampil |
| WASM | `cargo check --target wasm32-unknown-unknown` PASS |
| Web bundle | `trunk build --cargo-profile wasm-release` PASS; wasm 46 MB |

## Bukti perintah

```text
cargo test -p twg-map
  32 passed; 0 failed

cargo test -p twg-render
  5 passed; 0 failed

cargo test -p twg-server
  5 passed; 0 failed

cargo fmt --all --check
  PASS

cargo clippy --workspace --all-targets -- -D warnings
  PASS

cargo check -p twg-web --target wasm32-unknown-unknown
  PASS

trunk build --cargo-profile wasm-release
  ✅ success

node tools/web-stress.mjs --sample=10 \
  --wheel-test --pan-test --lod-toggle-test --select-test \
  --memory-test --fly-test --proxy-test
  9/9 PASS; minimum 47,19 FPS
  wheel/pan bearing 45/LOD toggle/select/memory/fly/proxy PASS

# proxy server-side
cargo run -p twg-server-app
curl http://127.0.0.1:8010/tiles/osm/19/429948/274783.png
  200 OK; panggilan kedua memakai disk cache
```

Log runtime native setelah perbaikan path aset dan pemilihan GLB yang valid
tidak menunjukkan error `bevy_asset` atau `bevy_gltf`.

## Struktur baru

`twg-map` dipecah menjadi:

- `geo.rs` — koordinat, Mercator, tile math, jarak, local offset.
- `tile.rs` — `TileSource`, `XyzTileSource`, `TileCover`, `cover_tiles`,
  `quadtree_cover`, dan template proxy.
- `camera.rs` — `MapCameraState`, `MapCursor`, `CameraFlight`, pan/zoom/rotate/pitch,
  world ↔ geo.

`twg-render` dipecah menjadi:

- `map/camera.rs` — komponen kamera dan transform.
- `map/input.rs` — mouse, wheel (Line/Pixel normalized), keyboard.
- `map/anchors.rs` — `GeoAnchor` Mercator dan update saat pan.
- `map/cursor.rs` — picking bidang peta.
- `map/tiles.rs` — tile world, spawn/despawn, transform.
- `map/tile_cache.rs` — LRU material/image per layer.
- `map/marker.rs` — marker GLTF, rotasi, dan geo anchor.
- `map/overlay.rs` — ribbon line, prism polygon, dan floor plan.
- `map/environment.rs` — ambient, directional light, fallback ground.
- `map/mod.rs` — plugin, resource adapter Bevy (`MapCameraResource`, `MapCursorResource`),
  `CameraFlightState`, dan `TileProxyConfig`.
- `stress/reload.rs` — reload aset/frame saat toggle full/LOD.
- `stress/selection.rs` — screen-space picking dan selection ring.
- `stress/memory.rs` — probe memori load/unload.

`twg-ui`:

- `compass_panel.rs` — kompas, tombol zoom, north, dan reset.

`twg-server`:

- `tiles.rs` — route tile proxy, disk cache, rate limit, dan test.

Web:

- `apps/web/tile-cache-sw.js` — cache-first tile OSM/Esri di Cache Storage.

## Gate G1

| Kriteria | Status |
|---|---|
| Peta raster tampil benar di native | PASS — screenshot di atas |
| Peta raster tampil di WASM | PASS — Chrome WebGL2, OSM dan Esri |
| Kamera pitch/bearing/zoom konsisten | PASS di native dan web, termasuk overzoom z23–z26 |
| Picking dan transform dasar | PASS — cursor lat/lon + marker selection ring |
| Target FPS dan memori tercapai | PASS — FPS web min 47,19; memori native dan web terukur |
| Tidak ada bug presisi koordinat terlihat | PASS pada zoom 13, 19, 23, dan 26 |
| Jalur fallback WebGL2 | PASS — WebGL2 adalah jalur aktual build web |
| Tidak ada blocker yang memaksa ganti stack | PASS |

## Keterbatasan dan pekerjaan lanjutan

- Uji web baru mencakup Chrome/Chromium WebGL2. Firefox, Safari, Edge, dan GPU
  integrated belum diukur. Jalankan `tools/web-stress.mjs` di mesin tersebut.
- Bundle WASM release 46 MB. `wasm-opt`, code splitting, dan kompresi Brotli
  masuk fase packaging.
- Memori sudah diukur, tetapi setelah clear RSS/WASM tidak turun kembali ke
  angka sebelum load (allocator/wgpu menahan buffer); perlu strategi unload
  GPU yang lebih agresif bila target memori ketat.
- Fly-to, kontrol kompas/zoom, quadtree tile cover, dan placeholder/error tile
  sudah diimplementasikan. Matriks browser penuh (Edge/Firefox/Safari) dan GPU
  integrated masih perlu diukur.
- Tile proxy server-side sudah ada (`/tiles/...`) dengan disk cache dan rate
  limit. Untuk produksi tetap disarankan menaruh proxy di depan provider agar
  attribution, quota, dan kebijakan cache terpusat.
- Model penuh 9,6k triangle tidak sanggup 10.000 marker di WebGL2. Stress web
  memakai LOD 2,4k triangle; build native default tetap model penuh dan bisa
  memilih LOD lewat `--stress-lod`.
- File `polwan_airud_*_draco.glb` gagal dimuat karena Bevy 0.20 belum mendukung
  `KHR_draco_mesh_compression`. Gunakan varian non-Draco atau tambahkan decoder
  Draco khusus bila ukuran unduhan menjadi prioritas.
- Object picking, selection ring, line, polygon, building extrusion, dan floor
  plan overlay sudah ada. Authoring interaktif (menggambar/mengedit dari UI)
  belum termasuk Fase 3.
- Animasi sudah berjalan lewat 20 frame hasil bake; playback skeletal penuh
  untuk marker individu masuk Fase 10.
- Rate limit OSM: cover tile maksimum 15×15 pada zoom rendah. Proxy server
  membatasi konkurensi dan interval permintaan; cache service worker dan disk
  cache mengurangi permintaan ulang. Attribution tetap harus terlihat.
- `WebAssetPlugin` mencetak peringatan keamanan saat startup. URL tile berasal
  dari template tetap, bukan input pengguna.

## Perintah menjalankan

```bash
cargo run -p twg-desktop
```

Desktop memakai `../../assets` relatif terhadap direktori kerja. Override
dengan:

```powershell
$env:TWG_ASSET_PATH = "F:\twgrev\assets"
cargo run -p twg-desktop
```

Web:

```bash
cd apps/web
trunk serve
```

Stress test web otomatis:

```bash
cd apps/web
trunk build --cargo-profile wasm-release
cd ../..
node tools/web-stress.mjs --sample=10 \
  --wheel-test --pan-test --lod-toggle-test --select-test \
  --memory-test --fly-test --proxy-test
```

Tile proxy server-side:

```bash
cargo run -p twg-server-app
# web: ?proxy=http://127.0.0.1:8010
# desktop: TWG_TILE_PROXY=http://127.0.0.1:8010
```
