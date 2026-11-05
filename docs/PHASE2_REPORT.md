# Laporan Fase 2 — Kontrak Domain dan DTO

**Tanggal:** 4 November 2026  
**Status:** selesai untuk kontrak baca/tulis utama, domain modular, dan golden contract tests.

## 1. Prinsip struktur

Fase 2 tidak memakai satu `lib.rs` besar. Setiap domain dipisah ke modul sendiri:

```text
crates/twg-domain/src/
├── lib.rs
├── access.rs          Role, SessionSubject, DataScope
├── error.rs           DomainError
├── geo.rs             GeoCoordinate, Rotation3, Scale3, ColorHex
└── models/
    ├── mod.rs
    ├── user.rs
    ├── satker.rs
    ├── operation.rs
    ├── scenario.rs
    ├── marker.rs
    ├── building.rs
    ├── line.rs
    ├── text_marker.rs
    ├── particle.rs
    ├── animation.rs
    ├── command.rs
    └── snapshot.rs

crates/twg-contracts/src/
├── lib.rs
├── envelope.rs
├── error.rs
├── time.rs
├── units.rs
├── auth.rs
├── users.rs
├── satkers.rs
├── operations.rs
├── scenarios.rs
├── categories.rs
├── alurs.rs
├── animations.rs
├── snapshots.rs
├── last_positions.rs
├── geocode.rs
├── uploads.rs
├── websocket.rs
├── paths.rs
└── map/
    ├── mod.rs
    ├── marker.rs
    ├── building.rs
    ├── line.rs
    ├── text_marker.rs
    └── particle.rs

crates/twg-contracts/tests/
├── golden_contract.rs
└── request_contracts.rs
```

## 2. Domain yang didefinisikan

- Role: `admin`, `admin_monitor`, `admin_satker`, `user_satker`, `user`.
- Scoping: `DataScope::{All, Satker, Unscoped}`.
- Session subject dan aturan akses satker.
- Entity: user, unit, satker, operasi, skenario, marker, building, line,
  text marker, particle, animasi, command, snapshot, last map, last position.
- Value object: koordinat, rotasi, skala, warna.
- Error domain.

## 3. Kontrak HTTP yang tersedia

| Modul | Isi |
|---|---|
| `envelope` | Single/List/Error/Message/Health response |
| `auth` | Login, refresh, logout, user identity, login response |
| `users` | User list, create/update, password, profile, session, online user |
| `satkers` | Satker response dan request |
| `operations` | Operasi, request, home operasi/skenario |
| `scenarios` | Skenario, create/update/name/copy request |
| `map::marker` | Marker + request geom/name/scale/altitude/rotation/duplicate |
| `map::building` | Building + request create/update/geom |
| `map::line` | Line + request create/update/geom |
| `map::text_marker` | Text marker + request create/update/geom |
| `map::particle` | Particle + request create |
| `animations` | Animation + route/spawn/combine/shoot/update/order request |
| `alurs` | Alur + request |
| `categories` | Category + request |
| `snapshots` | Snapshot + upsert/toggle request |
| `last_positions` | Last map, last position, combined response |
| `geocode` | Hasil geocoding |
| `uploads` | Field dan MIME yang diizinkan |
| `websocket` | Publish request, command payload, command name |
| `paths` | Path API statis dan dinamis |

## 4. Kompatibilitas yang ditangani

1. **`units` string versus array**: `UnitsPayload` menerima keduanya dan
   menyediakan `into_units()`.
2. **Timestamp berbeda**: response user/operasi/skenario memakai offset
   `+08:00`; response satker dan snapshot memakai naive timestamp. Tipe
   `Timestamp` dan `NaiveTimestamp` dipisahkan.
3. **`modelUrl` camelCase** di unit dipertahankan melalui `serde(rename)`.
4. **`satker_id` null dihilangkan**, bukan dikirim sebagai `null`.
5. **Home operation** memakai field JSON `operasi_id` dan `skenario_id`,
   sementara tipe Rust memakai nama English dengan `serde(rename)`.
6. **List total 0** tidak diserialisasi, sesuai envelope lama.
7. **Snapshot states** menerima JSON bebas melalui `serde_json::Value`.

## 5. Golden contract tests

`crates/twg-contracts/tests/golden_contract.rs` membaca langsung 16 fixture
Fase 0 dari `baseline/golden/redacted`:

- Login response.
- User list dengan `units` string.
- Satker list dengan naive timestamp.
- Operasi detail dan home operasi.
- Skenario detail dan list.
- Marker, building, line, text marker, particle.
- Animasi dan alur.
- Snapshot dan last position.
- Online user dan error 403.

Test request ada di `tests/request_contracts.rs`:

- Login/refresh.
- Marker create.
- Scenario create dengan indoor config.
- Snapshot upsert.
- Command payload.
- Spawn dan spawn pickup.

## 6. Verifikasi

| Pemeriksaan | Hasil |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| Test crate inti + server | **47 tests PASS** |
| Golden HTTP contract tests | **9 PASS** |
| Request contract tests | **8 PASS** |
| `cargo check` domain/contracts untuk `wasm32` | PASS |
| `cargo check`/WASM | PASS |
| `trunk build` | PASS |

Rincian 47 test:

| Crate/area | Jumlah |
|---|---:|
| `twg-auth` | 3 |
| `twg-client` | 2 |
| `twg-contracts` unit | 12 |
| Golden HTTP | 9 |
| Request contract | 8 |
| `twg-domain` | 6 |
| `twg-map` | 5 |
| `twg-server` | 2 |

## 7. Belum dikerjakan

- Golden response untuk endpoint write belum diambil; request diuji secara
  struktural, bukan diferensial terhadap server Go.
- Beberapa varian update animasi memakai tipe request generik; perlu dicek ulang
  saat handler Rust dibuat.
- Pemetaan domain → DTO → SQLx belum diimplementasikan.
- OpenAPI/TypeScript generation belum diperlukan karena client dan server Rust
  berbagi crate yang sama.
- Bug lama pada endpoint publik dan role middleware tetap ada di server Go dan
  belum diperbaiki.

## 8. Rekomendasi lanjutan

Fase 3 siap dimulai: spike `twg-map` native + WASM. Fokus:

1. Raster tile OSM/Esri tanpa API key.
2. Kamera zoom/pitch/bearing.
3. Quadtree tile dan cache.
4. Marker GLTF dan picking.
5. Batas memori/FPS untuk gate G1.

Server auth/read API dapat berjalan paralel setelah kontrak ini stabil.
