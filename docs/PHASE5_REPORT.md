# Laporan Fase 5 — Endpoint Read-Only

**Tanggal:** 6 November 2026

**Status:** PASS — seluruh endpoint read-only publik, privat, dan admin
dibandingkan dengan server Go lama pada database baseline yang sama. Hasil
differential: **193 check PASS, 0 FAIL, 3 skip terverifikasi golden**. New server
juga dibandingkan langsung dengan golden fixture (timestamp, null/omit, urutan,
dan `total`).

## Endpoint yang diimplementasikan

### Publik

| Method | Path | Data |
|---|---|---|
| GET | `/public/skenarios/{operasi_id}` | Scenario + operasi, `created_at ASC` |
| GET | `/public/skenarios/get/{id}` | Scenario tunggal |
| GET | `/public/markers/list/{skenario_id}` | Marker + operasi + skenario, `created_at DESC` |
| GET | `/public/text-markers/list/{skenario_id}` | Text marker, `created_at DESC` |
| GET | `/public/particles/list/{skenario_id}` | Particle, `created_at DESC` |
| GET | `/public/buildings/list/{skenario_id}` | Building, `created_at DESC` |
| GET | `/public/lines/list/{skenario_id}` | Line, `created_at DESC` |
| GET | `/public/alurs/list/{skenario_id}` | Alur, `created_at ASC` |
| GET | `/public/anims/list/{skenario_id}` | Animasi, `urut ASC`, exclude `is_combine`/`is_sequence` |
| GET | `/public/last_positions/{skenario_id}` | `last_map` + `last_position` |
| GET | `/public/list/operasis` | Home operasi + skenario |
| GET | `/public/get/home/operasis/{id}` | Home operasi tunggal |
| GET | `/public/users/online` | User dengan session aktif |
| GET | `/public/game-sessions/snapshot/{skenario_id}` | Snapshot atau `data: null` |

### Privat (bearer + session)

| Method | Path | Scoping |
|---|---|---|
| GET | `/private/operasis` | Pagination + search + satker |
| GET | `/private/operasis/{id}` | Satker policy |
| GET | `/private/home/operasis` | Satker policy |
| GET | `/private/users` | `admin_satker`: satker sendiri; lainnya: exclude `admin` |
| GET | `/private/satkers` | Semua role admin/satker; satker role hanya miliknya |
| GET | `/private/categories/all` | Semua kategori |
| GET | `/private/game-sessions/snapshot/{skenario_id}` | Snapshot |

### Admin (bearer + session + role)

| Method | Path | Role |
|---|---|---|
| GET | `/admin/categories` | `admin`, `admin_monitor`, `admin_satker` |
| GET | `/admin/categories/{id}` | `admin`, `admin_monitor`, `admin_satker` |

Pagination memakai `pageSize`/`pageNumber` 1-based, `search` case-insensitive,
dan `total` adalah jumlah setelah filter. `ListResponse` menghilangkan `total`
saat nol, sesuai kontrak lama.

## Implementasi

- `crates/twg-storage/src/read.rs` — `ReadStore`, query SQLx, mapping DTO,
  urutan list, timestamp WITA, dan `LastMap` kosong yang kompatibel dengan Go.
- `crates/twg-server/src/handlers/public.rs` — endpoint publik.
- `crates/twg-server/src/handlers/private.rs` — endpoint privat + pagination +
  satker policy.
- `crates/twg-server/src/handlers/admin.rs` — kategori admin.
- `AppState.read_store` — tersedia saat pool PostgreSQL terpasang; tanpa DB
  endpoint read mengembalikan 503 envelope.
- `RoleGuard` dipakai untuk route admin.
- `MapMode`, `BaseLayer`, `TextAlign`, role, dan JSON `indoor_config`/snapshot
  diparse ke tipe domain/kontrak.

Detail kompatibilitas:

- `LastMap` tanpa data mengembalikan objek nol seperti Go, termasuk timestamp
  `0001-01-01T00:00:00Z` dan `skenario_id` kosong.
- Snapshot yang tidak ada mengembalikan `{ "code": 200, "message": "success",
  "data": null }`.
- User online memakai session aktif, `last_used_at` terbaru, dan urutan session
  `last_used_at DESC NULLS LAST, created_at DESC`.
- Timestamp `TIMESTAMP` tanpa zona diperlakukan sebagai UTC lalu dikonversi ke
  WITA (+08:00), sama seperti golden.

## Bukti differential

Database: PostgreSQL 17 dari container lokal, schema 32 migrasi, seed baseline
(enam akun, tiga operasi, empat skenario, marker, building, line, text marker,
particle, anim, last position, snapshot).

Server yang dibandingkan:

- Go lama: `E:\tactical_game\server\server.exe`, port 8013.
- Rust: `target/debug/twg-server-app.exe`, port 8011.
- Keduanya memakai database dan `TOKEN_SYMMETRIC_KEY` yang sama.

Hasil:

```text
read parity: PASS (193 checks, 3 skipped)
```

Cakupan:

- 23 endpoint publik termasuk data kosong dan snapshot `null`.
- Empat role (`admin`, `admin_satker`, `user`, `user_satker`) untuk endpoint
  privat.
- Pagination halaman 1, halaman 2, halaman melewati total, search cocok, search
  kosong, dan kategori admin.
- Perbandingan status, `content-type`, keberadaan `X-Request-ID`, body JSON,
  urutan list, `total`, null/omit, dan timestamp.
- 30+ response dibandingkan langsung dengan file golden, bukan hanya dengan
  server Go yang berjalan di Windows.

Tiga check dilewati karena server Go lama **panic di zerolog** ketika menulis
error 403 di Windows tanpa tzdata `Asia/Makassar`:

```text
SKIP /private/operasis/{id} [legacy]
SKIP /private/satkers [legacy]
SKIP /private/satkers?active=true [legacy]
```

Ketiganya tetap dibandingkan dengan golden:

- `private-user-_private_operasis_...json` → `403 cannot access satker-assigned operasi`
- `private-user-_private_satkers.json` → `403 forbidden`

## Test otomatis

```text
cargo test -p twg-auth
  29 passed

cargo test -p twg-server
  33 passed

cargo test -p twg-storage
  0 passed (compile + integration via differential)

cargo fmt --all --check
  PASS

cargo clippy --workspace --all-targets -- -D warnings
  PASS
```

`tools/read-parity.mjs` dapat dijalankan ulang selama server Go/Rust dan
database baseline tersedia:

```bash
OLD_URL=http://127.0.0.1:8013 NEW_URL=http://127.0.0.1:8011 \
node tools/read-parity.mjs
```

## Perbedaan yang disengaja/diketahui

- Angka desimal dapat diserialisasi sebagai `15.0` (Rust) versus `15` (Go).
  Setelah JSON di-parse nilainya identik; perbandingan differential dan golden
  memakai nilai JSON, bukan byte mentah.
- Server Go di Windows tidak dapat mengembalikan error terstruktur karena
  zerolog panic saat `LoadLocation("Asia/Makassar")`; ini masalah lingkungan
  lama, bukan kontrak. Golden Linux dipakai sebagai referensi.
- `users/online` bersifat dinamis (session ID/timestamp); differential
  membandingkan hasil kedua server pada database dan session yang sama, bukan
  terhadap golden capture.
- Shadow traffic read-only belum dijalankan sebagai proses produksi; harness
  differential sudah menjadi dasar untuk itu.
