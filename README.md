# Tactical Game 3D — Rust

Migrasi client dan server Tactical Game 3D ke Rust.

- Server: Axum + Tokio + SQLx + PostgreSQL
- Client: Bevy 0.20 + egui
- Peta: crate `twg-map` dari awal
- Target: Windows, Linux, macOS, dan WebAssembly

Source lama di `E:\tactical_game` bersifat **read-only** dan tidak boleh diubah
dari workspace ini. Baseline, kontrak, fixture, dan hasil capture berada di
[`baseline/`](baseline/README.md).

## Status

Fase 1: workspace, batas crate, server health scaffold, desktop/web scaffold,
dan CI native + WASM.
Fase 2: kontrak domain/DTO modular dan golden contract tests.
Fase 3: spike `twg-map` — tile OSM dan Esri, overzoom sampai zoom 26,
kamera, cursor lat/lon, marker GLTF, overlay FPS, cache tile, dan stress test
1k/5k/10k marker. Native dan Chrome WebGL2 sudah diuji. Hasil web: 9/9 skenario
PASS, minimum 47,19 FPS pada 10.000 marker. Input web sudah stepwise (wheel
Line/Pixel dinormalisasi), pan mengikuti bearing kamera (diuji pada 45°),
marker memakai `GeoAnchor` agar ikut saat peta di-pan, dan tersedia tombol **Full 9.6k / LOD 2.4k** (shortcut `M`). Klik
marker memilih model dengan selection ring; memori native dan web sudah diukur
lewat `--memory-probe` / `?memory=1`.

Fase 3 juga mencakup fly-to (double-click, `N` ke utara), panel kompas/zoom,
quadtree tile cover, placeholder/error tile, tile proxy server-side dengan rate
limit, serta overlay line, polygon/building extrusion, dan floor plan. Detail
teknis dan angka terbaru ada di [laporan Fase 3](docs/PHASE3_REPORT.md).

`cargo check`/`build` WebAssembly dan `trunk build --cargo-profile
wasm-release` sudah berhasil. Bundle WASM release 46 MB; optimasi
`wasm-opt`/Brotli dikerjakan pada fase packaging.

Belum diimplementasikan pada fase ini:

- Auth PASETO/bcrypt.
- Query SQLx dan migrasi.
- Editor peta dan authoring line/polygon/floor plan.
- Animasi runtime penuh.
- UI editor.
- WebSocket server.
- Feature parity dengan React/Go.

## Struktur

```text
crates/
  twg-domain/          aturan role dan scoping satker
  twg-contracts/       envelope respons dan kontrak bersama
  twg-auth/            konfigurasi auth dan error
  twg-storage/         koneksi PostgreSQL/SQLx
  twg-server/          router Axum, health endpoint, dan tile proxy
  twg-client/          state client
  twg-map/             geo, tile source/cover, dan kamera peta
  twg-render/          plugin Bevy, tile OSM, marker, kamera
  twg-ui/              plugin egui dan overlay peta
  twg-asset-pipeline/  pipeline aset (scaffold)
apps/
  server/              binary server
  desktop/              binary native Bevy
  web/                  library WASM + Trunk
tools/
  web-stress.mjs       harness stress browser (Chrome DevTools Protocol)
```

Aturan dependency:

- `twg-domain` tidak boleh mengimpor Axum, SQLx, Bevy, atau browser API.
- `twg-contracts` tidak boleh mengimpor renderer.
- `twg-server` tidak boleh mengimpor Bevy.
- App client tidak boleh mengimpor crate server.

## Perintah

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p twg-domain -p twg-contracts -p twg-auth -p twg-map -p twg-client -p twg-server -p twg-render
cargo check -p twg-web --target wasm32-unknown-unknown
```

Server:

```bash
cargo run -p twg-server-app
```

Desktop:

```bash
cargo run -p twg-desktop
```

Desktop mencari aset di `../../assets` relatif terhadap direktori kerja.
Override dengan `TWG_ASSET_PATH` bila perlu.

Web:

```bash
cargo install trunk --locked
cd apps/web
trunk build
trunk serve
```

Stress test web otomatis memakai Chrome DevTools Protocol tanpa dependency npm:

```bash
cd apps/web
trunk build --cargo-profile wasm-release
cd ../..
node tools/web-stress.mjs --sample=10
```

`HTTP_SERVER_ADDRESS` dapat dioverride; default `127.0.0.1:8010`.

Tile proxy server-side tersedia di `GET /tiles/{layer}/{z}/{x}/{y}.png`
(`layer`: `osm` atau `satellite`) dengan disk cache di `.tile-cache/`. Client
web memakai route yang sama lewat `?proxy=<origin>`; desktop memakai
`TWG_TILE_PROXY=<origin>`. Bila tidak diset, client mengambil tile langsung
dari provider.

## Dependency policy

- Versi dependency di-pin melalui `Cargo.lock`.
- `cargo deny check` menggunakan [`deny.toml`](deny.toml).
- `cargo audit` dijalankan di CI.
- MapTiler tidak dipakai. Peta memakai OSM tanpa API key.

## Dokumen

- [Rekomendasi arsitektur dan library](docs/RUST_MIGRATION_LIBRARY_RECOMMENDATIONS.md)
- [Checklist implementasi](docs/RUST_IMPLEMENTATION_CHECKLIST.md)
- [Laporan Fase 1](docs/PHASE1_REPORT.md)
- [Laporan Fase 2](docs/PHASE2_REPORT.md)
- [Laporan Fase 3](docs/PHASE3_REPORT.md)
- [Laporan stress test web Gate G1](docs/WEB_STRESS_REPORT.md)
- [Baseline Fase 0](baseline/phase0-report.md)
- [Golden HTTP](baseline/golden/verification.generated.md)
- [WebSocket baseline](baseline/golden/websocket-capture.generated.md)
