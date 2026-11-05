# Laporan Fase 4 — Fondasi Server, Auth, dan Session

**Tanggal:** 6 November 2026

**Status:** PASS — fondasi Axum/PostgreSQL dan auth Rust berjalan. 29 test
`twg-auth` + 32 test `twg-server` lulus, migrasi SQLx berhasil dari database
kosong, smoke test PostgreSQL lulus, dan parity PASETO/session diuji dua arah
dengan server Go lama.

**Keputusan:** lanjut ke Fase 5. Client lama dapat memakai access/refresh token
yang sudah ada; tidak perlu forced logout.

## Yang diimplementasikan

### Fondasi server

| Area | Implementasi |
|---|---|
| Config | `app.env`/`.env` + environment process; `DB_SOURCE`/`DATABASE_URL`, `TOKEN_SYMMETRIC_KEY`, TTL, role single-device, CORS, body limit, uploads, rate limit |
| Tracing | `tracing-subscriber` dengan `EnvFilter`; span HTTP berisi request ID, method, path |
| Request ID | `X-Request-ID` dipakai ulang atau dibuat baru dan dikembalikan pada response |
| Error | Envelope `{ "code": <HTTP>, "message": "..." }`; error internal tidak membocorkan detail |
| PostgreSQL | `PgPool` + readiness `SELECT 1` |
| Graceful shutdown | Ctrl-C + SIGTERM (Unix) |
| CORS | Allowlist `CORS_DOMAIN`, preflight, credentials untuk origin eksplisit |
| Body limit | `DefaultBodyLimit` + header `X-Max-Body-Bytes` |
| Static upload | `/uploads` memakai `ServeDir`, traversal ditolak, `X-Content-Type-Options: nosniff` |
| Rate limit | Fixed window per IP untuk login/refresh, response 429 + `Retry-After` |
| Migrasi | 32 migrasi SQLx (31 migrasi lama + extension `uuid-ossp`) |

`run_migrations` mendeteksi database baseline lama: jika `users` sudah ada
tetapi migrasi inti belum pernah dicatat SQLx, migrasi dilewati agar skema lama
tidak diubah. Database kosong menjalankan seluruh migrasi.

### Auth

| Area | Implementasi |
|---|---|
| Password | Verifikasi bcrypt legacy; hash baru memakai bcrypt cost 10 |
| Token | PASETO v2.local XChaCha20-Poly1305 |
| Claims | `id`, `username`, `role`, `satker_id`, `token_type`, `issued_at`, `expired_at`, `exp` |
| Access token | TTL default 24 jam |
| Refresh token | TTL default 168 jam, disimpan sebagai `SHA-256(token + ":" + key)` |
| Session | Tabel `user_sessions`; validasi berdasarkan user + device |
| Single-device | `admin`/`admin_satker`; device kedua ditolak 403 |
| Multi-device | Upsert per `(user, device)` |
| Refresh rotation | `UPDATE ... WHERE refresh_token_hash = old` atomik; replay ditolak |
| Identify | Mengembalikan pasangan token baru dan merotasi refresh hash |
| Logout | Revoke session user + device |
| Password change | Verifikasi password lama, hash baru, update `users` |
| Role middleware | `RoleGuard` + `AuthContext::require_roles` |
| Satker policy | `Role`/`DataScope`/`SessionSubject` di `twg-domain` |

## Endpoint

| Method | Path | Auth | Perilaku |
|---|---|---|---|
| `POST` | `/auth/login` | public | HTTP 201, body `code: 200`, user + token |
| `POST` | `/auth/refresh` | public | Rotasi access + refresh |
| `POST` | `/auth/logout` | bearer | Revoke session |
| `GET` | `/private/identify` | bearer + device | User + token baru |
| `PUT` | `/private/profile/password` | bearer + device | Ganti password |
| `GET` | `/healthz` | public | Liveness |
| `GET` | `/readyz` | public | Ping PostgreSQL |
| `GET` | `/tiles/{layer}/{z}/{x}/{y}` | public | Tile proxy |
| `GET` | `/uploads/*` | public | Static upload aman |

`X-Device-ID` di header lebih diprioritaskan daripada `device_id` di body.
Login/refresh/logout menolak device kosong dengan `device_id is required`.

## Kompatibilitas token lama

Token Go memakai `o1egl/paseto` v1.0.0. Dua detail penting yang harus ditiru:

1. Associated data adalah `preAuthEncode(header, nonce, footer)`, yaitu jumlah
   piece little-endian, lalu panjang + isi setiap piece. Bukan header mentah.
2. Saat footer `nil`, `infToByteArr(nil)` menghasilkan JSON `null`; token lama
   membawa footer `"null"` (`bnVsbA`). Decoder Rust mendukung footer opsional
   dan memasukkannya ke AAD.

Hasil uji dua arah dengan database baseline dan key yang sama:

```text
PASS legacy login
PASS Rust accepts legacy refresh token
PASS Rust accepts legacy access token
PASS rust login
PASS legacy server accepts Rust access token
PASS legacy server accepts Rust refresh token
PASETO/session parity: PASS
```

Uji yang sama dijalankan untuk `baselineadmin` (single-device) dan
`baselineuser` (multi-device).

## Bukti perintah

```text
cargo test -p twg-auth
  29 passed; 0 failed

cargo test -p twg-server
  32 passed; 0 failed

cargo fmt --all --check
  PASS

cargo clippy --workspace --all-targets -- -D warnings
  PASS

Database kosong:
  readyz=200
  _sqlx_migrations count = 32
  users + user_sessions = ada

node tools/phase4-smoke.mjs
  PASS healthz
  PASS readyz
  PASS login status
  PASS login envelope
  PASS login tokens
  PASS single-device conflict
  PASS identify
  PASS identify rotates refresh
  PASS pre-identify refresh rejected
  PASS refresh
  PASS refresh rotates
  PASS refresh replay rejected
  PASS logout
  PASS session revoked
  PASS wrong password
  phase 4 auth smoke: PASS
```

## Konfigurasi

Lihat [`.env.example`](../.env.example). Variabel utama:

| Variabel | Default | Catatan |
|---|---|---|
| `DB_SOURCE`/`DATABASE_URL` | wajib | PostgreSQL DSN |
| `TOKEN_SYMMETRIC_KEY` | wajib | tepat 32 byte |
| `ACCESS_TOKEN_DURATION` | `24h` | format `s`/`m`/`h`/`d` |
| `REFRESH_TOKEN_DURATION` | `168h` | format yang sama |
| `SINGLE_DEVICE_ROLES` | `admin,admin_satker` | comma-separated |
| `CORS_DOMAIN` | `http://localhost:5173` | comma-separated atau `*` |
| `MAX_BODY_BYTES` | `104857600` | minimal 1024 |
| `UPLOADS_DIR` | `uploads` | static upload |
| `AUTH_RATE_LIMIT` | `10` | 0 menonaktifkan |
| `AUTH_RATE_WINDOW_SECONDS` | `60` | window rate limit |

## Keamanan

- Debug config meredaksi `TOKEN_SYMMETRIC_KEY` dan database URL.
- Token, password, dan refresh hash tidak pernah dilog.
- Password baru dibatasi 8–72 byte (batas bcrypt).
- Error internal hanya mengembalikan `internal server error` ke client.
- Static upload tidak mengizinkan traversal dan memakai `nosniff`.
- Refresh token disimpan sebagai hash, bukan plaintext.
- Rotasi refresh atomik mencegah dua request paralel memakai token yang sama.

## Keterbatasan dan pekerjaan lanjutan

- Rehash Argon2id setelah login bcrypt belum diterapkan (opsional).
- Rate limiter masih in-process; deployment multi-instance perlu Redis/limiter
  terpusat.
- Heartbeat belum berupa endpoint terpisah; setiap request privat memperbarui
  `last_used_at`, dan expiry mengikuti TTL token.
- Endpoint data read/write dan enforcement satker per query masuk Fase 5–6.
- `identify` sengaja merotasi refresh hash. Server Go lama mengembalikan refresh
  token baru tanpa menyimpannya, sehingga token itu tidak bisa dipakai; perilaku
  Rust memperbaikinya. Client harus memakai token dari response `identify`.
- Uji beban auth, audit log, dan WebSocket belum termasuk Fase 4.

## Menjalankan

```bash
cp .env.example app.env
# isi DB_SOURCE dan TOKEN_SYMMETRIC_KEY lokal
cargo run -p twg-server-app
```

Smoke test:

```bash
BASE_URL=http://127.0.0.1:8010 \
TWG_USERNAME=baselineadmin TWG_PASSWORD='Baseline#2026' \
node tools/phase4-smoke.mjs
```

Parity dengan server Go lama (kedua server memakai database baseline dan key
yang sama):

```bash
OLD_URL=http://127.0.0.1:8013 NEW_URL=http://127.0.0.1:8011 \
node tools/paseto-parity.mjs
```
