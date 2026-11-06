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

Fase 4: fondasi server Axum + PostgreSQL — config `app.env`/environment,
request ID, error envelope, CORS allowlist, body limit, readiness DB, graceful
shutdown, rate limit login/refresh, dan migrasi SQLx. Auth memakai bcrypt legacy,
PASETO v2.local, access/refresh token, session per device (single-device untuk
`admin`/`admin_satker`), rotasi refresh single-flight, identify, logout, dan
password change. Parity token diuji dua arah dengan server Go lama. Detail ada di
[laporan Fase 4](docs/PHASE4_REPORT.md).

Fase 5: endpoint read-only — 14 endpoint publik (skenario, marker, text marker,
particle, building, line, alur, anim, last position, home operasi, user online,
snapshot), tujuh endpoint privat dengan satker policy, dan dua endpoint admin
category. Pagination `pageSize`/`pageNumber`, search case-insensitive, mapping
DTO, dan timestamp WITA mengikuti server Go. Differential terhadap server Go
lama pada database baseline yang sama: **193 check PASS, 0 FAIL, 3 skip** yang
tervalidasi golden. Detail ada di [laporan Fase 5](docs/PHASE5_REPORT.md).

Fase 6: write API dan upload — CRUD user, satker, operasi, skenario (termasuk
copy dengan remap id JSON), marker, building, line, text marker, alur, seluruh
tipe animasi, particle/shoot, snapshot, dan command `last_map`/`last_position`.
Operasi multi-tabel memakai transaksi. Upload gambar/video memvalidasi MIME dari
magic bytes dengan `infer`, membatasi ukuran, menyanitasi nama file, dan
menyajikan `/uploads` dengan header aman. Differential write terhadap server Go
lama: **89 check PASS** plus 8 safety check. Detail ada di
[laporan Fase 6](docs/PHASE6_REPORT.md).

Fase 7: realtime `twg.ws.v1` — tiket WebSocket sekali pakai 30 detik, upgrade
`/ws`, channel per skenario dengan otorisasi satker, sequence, idempotency,
ack/error, presence, heartbeat, resume dari history, resync dengan snapshot
fallback, dan dual-publish Centrifugo opsional. Smoke test end-to-end PASS
(termasuk cross-satker, duplicate command, fan-out 8 klien, dan reconnect storm).
Spesifikasi ada di [protokol realtime](docs/REALTIME_PROTOCOL.md), laporan di
[laporan Fase 7](docs/PHASE7_REPORT.md).

Fase 8: client shell Bevy + egui — state layar, tema dan widget, HTTP client
typed dengan refresh single-flight, transport native (`ureq`/`tungstenite`) dan
web (`fetch`/`WebSocket`), penyimpanan sesi, serta layar Login, Dashboard,
Operasi, Skenario, Latihan, Pengguna, Satker, Online, Snapshot, Profil, dan
Preview. Alur non-peta diverifikasi di desktop dan Chrome headless; detail dan
deviasi ada di [laporan Fase 8](docs/PHASE8_REPORT.md).

`cargo check`/`build` WebAssembly dan `trunk build --cargo-profile
wasm-release` sudah berhasil. Bundle WASM release 46 MB; optimasi
`wasm-opt`/Brotli dikerjakan pada fase packaging.

Belum diimplementasikan pada fase ini:

- Editor peta dan authoring line/polygon/floor plan.
- Animasi runtime penuh.
- Upload gambar/video dari client shell.
- Feature parity dengan React/Go.

## Struktur

```text
crates/
  twg-domain/          aturan role dan scoping satker
  twg-contracts/       envelope respons dan kontrak bersama
  twg-auth/            bcrypt, PASETO v2.local, session, dan auth service
  twg-storage/         koneksi PostgreSQL/SQLx, migrasi, auth store, dan read store
  twg-server/          router Axum, auth, middleware, health, dan tile proxy
  twg-client/          state client
  twg-map/             geo, tile source/cover, dan kamera peta
  twg-render/          plugin Bevy, tile OSM, marker, kamera
  twg-ui/              plugin egui dan overlay peta
  twg-asset-pipeline/  pipeline aset (scaffold)
apps/
  server/              binary server
  desktop/              binary native Bevy
  web/                  library WASM + Trunk
docker/
  centrifugo/          config Centrifugo untuk testing lokal
compose.yaml           PostgreSQL + Centrifugo
```
tools/
  web-stress.mjs       harness stress browser (Chrome DevTools Protocol)
  client-smoke.mjs     smoke client shell web (sesi, API, WebSocket, screenshot)
  phase4-smoke.mjs     smoke test auth terhadap server + PostgreSQL
  paseto-parity.mjs    parity token Rust vs server Go lama
  read-parity.mjs      differential endpoint read-only Rust vs server Go lama
  write-parity.mjs     differential endpoint write + safety check Rust vs Go
  ws-smoke.mjs         smoke test WebSocket twg.ws.v1 end-to-end
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
cargo test -p twg-domain -p twg-contracts -p twg-auth -p twg-storage -p twg-map -p twg-client -p twg-server -p twg-render
cargo check -p twg-web --target wasm32-unknown-unknown
```

Server:

```bash
cp .env.example .env   # sesuaikan DB_SOURCE dan TOKEN_SYMMETRIC_KEY
cargo run -p twg-server-app
```

`.env` dibaca dari working directory; environment process tetap menang. File
lama `app.env` masih didukung dan menimpa `.env` bila ada.
`TOKEN_SYMMETRIC_KEY` wajib tepat 32 byte. `readyz` melakukan ping PostgreSQL;
`GET /tiles/...` tetap tersedia untuk proxy tile.

### PostgreSQL dan Centrifugo lokal (Docker Desktop)

```bash
docker compose up -d
docker compose ps
```

`compose.yaml` menjalankan:

| Service | Container | Address |
|---|---|---|
| PostgreSQL 16 | `twg-db-local` | `127.0.0.1:55432` |
| Centrifugo 6.10 | `twg-centrifugo-local` | `127.0.0.1:8000` |

Nilai database diambil dari `.env` (`POSTGRES_USER`, `POSTGRES_PASSWORD`,
`POSTGRES_DB`, `POSTGRES_PORT`). Volume data bernama `twg_twg-db-data`;
reset total dengan `docker compose down -v`.

Setelah server pertama kali berjalan (migrasi SQLx otomatis), isi fixture
baseline:

```bash
docker exec -i twg-db-local psql -U twg_baseline -d twg_baseline \
  -v ON_ERROR_STOP=1 -f - < baseline/fixtures/seed.sql
```

Centrifugo untuk testing dual-publish:

- HTTP API: `http://127.0.0.1:8000/api`, header `X-API-Key: twg-centrifugo-test-key`
- Health: `http://127.0.0.1:8000/health`
- Channel lama: `twg-jagradewata`
- Config: `docker/centrifugo/config.json`

Config testing mengaktifkan `client.insecure` dan `allowed_origins: ["*"]`;
jangan dipakai di produksi. `CENTRIFUGO_DUAL_PUBLISH=true` di `.env` membuat
server Rust ikut mem-publish event ke channel tersebut.

Desktop:

```bash
cargo run -p twg-desktop
```

Desktop membuka client shell dan memakai `TWG_API_URL` (default
`http://127.0.0.1:8010`). Gunakan `--stress ...` atau `TWG_DEMO=1` untuk
menjalankan map demo tanpa shell. Desktop mencari aset di `../../assets`
relatif terhadap direktori kerja; override dengan `TWG_ASSET_PATH`.

Web:

```bash
cargo install trunk --locked
cd apps/web
trunk build
trunk serve
```

Web memakai origin halaman sebagai API default; tambahkan `?api=<origin>` untuk
API lain. Parameter `?stress=`, `?benchmark=`, atau `?memory=` tetap menjalankan
map demo untuk harness Gate G1.

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

Realtime memakai `POST /private/ws-ticket` lalu `GET /ws?ticket=...`.
`WS_TICKET_TTL_SECONDS` mengatur umur tiket (default 30 detik) dan
`WS_HISTORY_LIMIT` mengatur jumlah event yang disimpan untuk resume (default
256). Dual-publish ke channel Centrifugo lama diaktifkan dengan
`CENTRIFUGO_DUAL_PUBLISH=true` beserta `CENTRIFUGO_ADDR`, `CENTRIFUGO_KEY`, dan
`CHANNEL`. Detail protokol ada di [protokol realtime](docs/REALTIME_PROTOCOL.md).

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
- [Laporan Fase 4](docs/PHASE4_REPORT.md)
- [Laporan Fase 5](docs/PHASE5_REPORT.md)
- [Laporan Fase 6](docs/PHASE6_REPORT.md)
- [Laporan Fase 7](docs/PHASE7_REPORT.md)
- [Laporan Fase 8](docs/PHASE8_REPORT.md)
- [Protokol realtime](docs/REALTIME_PROTOCOL.md)
- [Baseline Fase 0](baseline/phase0-report.md)
- [Golden HTTP](baseline/golden/verification.generated.md)
- [WebSocket baseline](baseline/golden/websocket-capture.generated.md)
