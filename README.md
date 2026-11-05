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
Fase 3: spike `twg-map` — tile OSM, kamera, cursor lat/lon, marker GLTF
non-Draco, dan overlay FPS. Native sudah berjalan; WASM compile dan
`trunk build` lulus. Runtime browser belum diuji.

`cargo check`/`build` WebAssembly dan `trunk build` sudah berhasil. Bundle web
masih debug dan berukuran besar; optimasi release dikerjakan pada fase
packaging.

Belum diimplementasikan pada fase ini:

- Auth PASETO/bcrypt.
- Query SQLx dan migrasi.
- Object picking/selection dan editor peta.
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
  twg-server/          router Axum dan health endpoint
  twg-client/          state client
  twg-map/             geo, tile source/cover, dan kamera peta
  twg-render/          plugin Bevy, tile OSM, marker, kamera
  twg-ui/              plugin egui dan overlay peta
  twg-asset-pipeline/  pipeline aset (scaffold)
apps/
  server/              binary server
  desktop/              binary native Bevy
  web/                  library WASM + Trunk
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
cargo test -p twg-domain -p twg-contracts -p twg-auth -p twg-map -p twg-client -p twg-server
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

Desktop mencari aset di `../../assets` relatif terhadap executable. Override
dengan `TWG_ASSET_PATH` bila perlu.

Web:

```bash
cargo install trunk --locked
cd apps/web
trunk build
trunk serve
```

Runtime browser belum diverifikasi otomatis karena `agent-browser` belum
terpasang di mesin pengembangan.

`HTTP_SERVER_ADDRESS` dapat dioverride; default `127.0.0.1:8010`.

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
- [Baseline Fase 0](baseline/phase0-report.md)
- [Golden HTTP](baseline/golden/verification.generated.md)
- [WebSocket baseline](baseline/golden/websocket-capture.generated.md)
