# Laporan Fase 6 — Write API dan Upload

**Tanggal:** 6 November 2026

**Status:** PASS — seluruh endpoint write (kategori, satker, user, operasi,
skenario, marker, building, line, text marker, alur, animasi, snapshot, command
last map/position) dan upload media sudah diimplementasikan. Differential
terhadap server Go lama: **89 check PASS**, ditambah **8 safety check** Rust-only
untuk validasi, not-found, konflik, dan upload.

## Endpoint yang diimplementasikan

### Admin

| Method | Path |
|---|---|
| POST/PUT/DELETE | `/admin/categories`, `/admin/categories/{id}` |
| POST/PUT/DELETE | `/admin/operasis`, `/admin/operasis/{id}` |
| POST | `/admin/skenarios` |
| POST | `/admin/skenarios/copy` |
| PUT | `/admin/skenarios/{id}`, `/admin/skenarios/name/{id}` |
| DELETE | `/admin/skenarios/{id}` |
| POST | `/admin/anims/urut`, `/admin/anims/urut/v2` |
| POST/PUT/DELETE | `/admin/alurs`, `/admin/alurs/{id}` |
| POST/PUT/DELETE | `/admin/users`, `/admin/users/{id}` |
| POST/PUT/DELETE | `/admin/satkers`, `/admin/satkers/{id}` |

### Privat

| Area | Endpoint |
|---|---|
| Profil | `PUT /private/profile` |
| Upload | `POST /private/upload-image`, `POST /private/upload-video` |
| Command | `POST /private/socket` (simpan `last_map`/`last_position`) |
| Marker | create, geom, name, scale, altitude, rotasi, duplicate, delete |
| Text marker | create, update, geom, delete |
| Building | create, update, geom, delete |
| Line | create, update, geom, delete |
| Animasi | route/route v2, spawn, spawn_pickup, remove, combine/combine v2, shoot/shoot v2, spawn_particle, remove_particle |
| Animasi update | route data/v2, route geom/v2, spawn data, particle data, remove data, remove_particle data, combine data, shoot data, combine/delete |
| Animasi delete | route, spawn, combine, shoot, particle |
| Snapshot | `PUT/PATCH/DELETE /private/game-sessions/snapshot/...` (admin & admin_satker) |

### Publik

| Method | Path |
|---|---|
| POST | `/public/users/{id}/kick-session` (kontrak lama tanpa auth) |

## Transaksi dan konsistensi

Transaksi PostgreSQL dipakai untuk operasi yang menyentuh lebih dari satu baris:

- `create_spawn`: marker spawn + anim, termasuk update altitude.
- `create_particle`: particle + anim `spawn_particle`.
- `create_combine`: update tipe child + baris parent.
- `create_shoot`: update `markers.shoot` + anim.
- `update_combine_data`: update tipe child yang masuk/keluar + data parent.
- `update_shoot_data`: pindah/filter shoot pada satu atau dua marker + anim.
- `delete_spawn` / `delete_shoot` / `delete_combine` / `delete_particle`:
  membersihkan referensi marker/particle lalu menghapus anim.
- `copy_scenario`: skenario + marker + building + line + text marker + particle +
  anim, dengan remap id di dalam `data`/`geom` JSON.
- `update_anim_urut` / urut v2: batch update `urut` dalam satu transaksi.
- `save_command`: insert command + upsert `last_maps`/`last_positions`.
- Update operasi dengan `satker_id`: cascade ke seluruh skenario.

Setiap kegagalan di tengah transaksi di-rollback otomatis oleh SQLx.

## Upload

- Multipart field `image` dan `video`.
- MIME dideteksi dari magic bytes memakai crate `infer`, bukan header klien.
- Tipe yang diterima: JPEG/PNG/GIF/WebP dan MP4/WebM/Ogg/QuickTime.
- Batas ukuran: 20 MB gambar, 200 MB video (di samping `MAX_BODY_BYTES`).
- Nama file dibuat ulang: `<epoch-millis>-<nama-aman>`. Path traversal, spasi,
  dan karakter kontrol disanitasi.
- Abstraksi `UploadStorage` dengan implementasi `LocalUploadStorage`; object
  storage dapat ditambahkan tanpa mengubah handler.
- `/uploads` disajikan dengan `Content-Security-Policy: default-src 'none';
  sandbox` dan `Cross-Origin-Resource-Policy: same-site`, plus `nosniff` global.

## Bukti differential

Database PostgreSQL 17, schema 32 migrasi, seed baseline. Server Go lama
(`E:\tactical_game\server\server.exe`, port 8013) dan Rust
(`target/debug/twg-server-app.exe`, port 8011) memakai database dan
`TOKEN_SYMMETRIC_KEY` yang sama. Database di-reset dari seed sebelum tiap pass.
Publish command Go diarahkan ke fake Centrifugo agar respons 201 dapat
dibandingkan.

```text
write parity: PASS (89 checks, 8 safety checks)
```

Cakupan differential:

- 89 request identik per server: login, kategori, satker, user, operasi,
  skenario (create/update/name/copy/delete), marker (semua update + duplicate),
  building, line, text marker, alur, seluruh tipe animasi beserta update/delete,
  snapshot upsert/toggle/read/delete, command ZOOM/MOVE/ROT, last positions,
  seluruh endpoint read terkait, dan upload gambar.
- Respons dibandingkan setelah normalisasi id, timestamp, token, dan prefiks
  nama file upload. Urutan key JSON diabaikan.
- Safety check Rust: duplicate username 409, marker tidak ditemukan 404,
  UUID path invalid 400, kategori tidak ditemukan 404, combine kosong 400,
  toggle snapshot tidak ada 404, MIME upload salah 400, field upload hilang 400.

`tools/write-parity.mjs` dapat dijalankan ulang:

```bash
OLD_URL=http://127.0.0.1:8013 NEW_URL=http://127.0.0.1:8011 \
node tools/write-parity.mjs
```

## Perbedaan yang disengaja

- **Create/update user tidak mengembalikan `password` dan `unit_id`.** Server Go
  mengembalikan baris database mentah sehingga hash bcrypt bocor ke klien. Rust
  memakai DTO `User` yang aman. Ini perbaikan keamanan yang disengaja.
- **Create skenario oleh `admin_satker` mengembalikan `operasi` lengkap.** Server
  Go memakai mapper sederhana yang mengisi objek `operasi` dengan nilai nol.
  Rust mengembalikan data operasi yang sebenarnya; field lain identik.
- **Validasi upload memakai magic bytes** dan batas ukuran per jenis. Server Go
  hanya mempercayai `Content-Type` dari multipart dan batas global 1 GB.
- **Path UUID invalid** ditolak oleh extractor Axum dengan status 400 dan body
  teks, bukan envelope JSON. Status tetap sama dengan Go.
- **Endpoint v2** menyimpan JSON hasil serialisasi ulang (kompak) dan bukan byte
  mentah klien. Nilai dan makna JSON identik; format angka mengikuti gaya Go.
- **`POST /private/socket`** menyimpan state dan mengembalikan
  `{"result":{}}`; fan-out real-time baru tersedia pada Fase 7.
- **Idempotency tambahan** tidak ditambahkan karena tidak ada di kontrak lama.
  `PUT`/`DELETE` sudah idempoten secara alami.
- **Audit perubahan** belum diaktifkan karena belum ada persetujuan desain.

## Test otomatis

```text
cargo test -p twg-storage
  2 passed

cargo test -p twg-server
  35 passed

cargo fmt --all --check
  PASS

cargo clippy --workspace --all-targets -- -D warnings
  PASS
```
