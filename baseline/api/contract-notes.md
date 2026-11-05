# Catatan Kontrak API

Dokumen ini merangkum kontrak yang harus dipertahankan saat server Rust
dibangun. Sumber utama tetap kode Go dan golden response yang akan diambil
melalui `scripts/capture-golden.ps1`.

## 1. Envelope

Single:

```json
{
  "code": 200,
  "message": "success",
  "data": {}
}
```

List:

```json
{
  "code": 200,
  "message": "success",
  "data": [],
  "total": 0
}
```

Error:

```json
{
  "code": 400,
  "message": "error message"
}
```

Status HTTP aktual harus sama dengan `code` pada body untuk endpoint yang
menggunakan envelope.

## 2. Auth

### Login

`POST /auth/login`

```json
{
  "username": "baselineadmin",
  "password": "Baseline#2026",
  "device_id": "device-1"
}
```

Respons sukses HTTP 201:

```json
{
  "code": 201,
  "message": "success",
  "data": {
    "user": {
      "id": "uuid",
      "name": "string",
      "username": "string",
      "role": "admin",
      "satker_id": "uuid-or-omitted",
      "profile_img": "string",
      "units": [],
      "created_at": "RFC3339",
      "updated_at": "RFC3339"
    },
    "access_token": "paseto-v2-local",
    "refresh_token": "paseto-v2-local",
    "access_token_expires_at": "RFC3339",
    "refresh_token_expires_at": "RFC3339"
  }
}
```

Catatan:

- `device_id` dapat dikirim lewat body atau header `X-Device-ID`.
- Header lebih diprioritaskan.
- Password memakai bcrypt.
- Token memakai PASETO v2.local dengan kunci simetris 32 byte.
- Refresh token disimpan sebagai `SHA-256(refresh_token + ":" + secret)`.
- Role `admin` dan `admin_satker` memakai single-device session.
- `units` pada login adalah array objek.
- `units` pada list/detail user adalah string JSON. Ini perbedaan bentuk yang
  harus dipertahankan atau dinormalisasi dengan versi API baru.

### Refresh

`POST /auth/refresh`

```json
{
  "refresh_token": "paseto",
  "device_id": "device-1"
}
```

### Logout

`POST /auth/logout` dengan Bearer token dan body `{ "device_id": "device-1" }`.

### Identify

`GET /private/identify` dengan Bearer token dan `X-Device-ID`.

## 3. Role dan scoping

| Role | Akses data |
|---|---|
| `admin` | Semua data |
| `admin_monitor` | Semua data; beberapa aksi tulis dibatasi di handler |
| `admin_satker` | Data dengan `satker_id` miliknya |
| `user_satker` | Data dengan `satker_id` miliknya |
| `user` | Hanya data dengan `satker_id IS NULL` |

Aturan ini harus ditegakkan di server dan diuji untuk setiap endpoint, termasuk
detail, list, command, snapshot, upload, dan relasi antar-entity.

## 4. Pagination

Query:

```text
pageSize=10&pageNumber=1&search=
```

`pageNumber` adalah **1-based**. Offset dihitung sebagai:

```text
offset = (pageNumber - 1) * pageSize
```

`pageSize` dan `pageNumber` wajib untuk endpoint yang memakai pagination.
`search` bersifat opsional.

## 5. Timestamp dan angka

- Server mengubah waktu ke zona `Asia/Makassar` (WITA) melalui
  `utils.ToWitaTimezone`.
- JSON timestamp diserialisasi sebagai RFC3339.
- UUID diserialisasi sebagai string.
- Koordinat latitude/longitude disimpan dalam `DOUBLE PRECISION` dan
  diserialisasi sebagai number.
- Nilai nullable menggunakan pointer/omitempty pada DTO; jangan mengubah null
  menjadi string kosong tanpa keputusan eksplisit.

## 6. Upload

`POST /private/upload-image` dengan field multipart `image`.  
`POST /private/upload-video` dengan field multipart `video`.

Respons sukses:

```json
{
  "code": 200,
  "message": "success",
  "data": "/uploads/<generated-name>"
}
```

Tipe yang diterima saat ini:

- Image: `image/jpeg`, `image/jpg`, `image/png`, `image/gif`, `image/webp`
- Video: `video/mp4`, `video/webm`, `video/ogg`, `video/quicktime`

Temuan: validasi hanya memakai header `Content-Type`, nama file asli masih
dipakai, dan batas body default/contoh sangat besar. Lihat
`security/findings.md`.

## 7. Endpoint penting

| Domain | Endpoint utama |
|---|---|
| Auth | `/auth/login`, `/auth/refresh`, `/auth/logout`, `/private/identify` |
| Operasi | `/private/operasis`, `/private/home/operasis`, `/admin/operasis` |
| Skenario | `/public/skenarios/*`, `/admin/skenarios/*` |
| Peta | `/private/markers/*`, `/private/buildings/*`, `/private/lines/*`, `/private/text-markers/*` |
| Animasi | `/private/anims/*`, `/public/anims/list/:skenario_id` |
| Snapshot | `/public/game-sessions/snapshot/:id`, `/private/game-sessions/snapshot/:id` |
| Realtime | `POST /private/socket` |
| Upload | `/private/upload-image`, `/private/upload-video` |
| Admin | `/admin/users`, `/admin/satkers`, `/admin/categories` |

Inventaris lengkap: [routes.generated.csv](routes.generated.csv) dan
[routes.generated.md](routes.generated.md).

## 8. Temuan dari golden response

- Login: HTTP **201**, body `code: 200`. Ini perbedaan yang harus dipertahankan
  atau dinormalisasi melalui versi API baru.
- Timestamp memakai offset `+08:00` (WITA).
- `units` pada login adalah array; pada list user adalah string JSON `"[]"`.
- `satker_id` null dihilangkan dari JSON, bukan dikirim sebagai `null`.
- `indoor_config` kosong diserialisasi sebagai objek `{}`.
- User role biasa mendapat 403 pada admin route dan `/private/satkers`.
- User role biasa mendapat **200** pada `/private/users`, sehingga dapat
  melihat daftar user lain. Ini dicatat sebagai temuan keamanan S13.
- `/public/users/online` mengembalikan data sesi lengkap tanpa autentikasi.

Golden response dan verifikasi ada di
[`../golden/verification.generated.md`](../golden/verification.generated.md).

## 9. Implementasi Rust

Kontrak Rust sudah tersedia di `crates/twg-contracts` dan dipecah per resource:
`auth`, `users`, `satkers`, `operations`, `scenarios`, `map/*`, `animations`,
`snapshots`, `last_positions`, `websocket`, dan `paths`. Domain terpisah di
`crates/twg-domain`.

Golden contract test membaca fixture dari folder ini dan memvalidasi:

- envelope,
- login dan user list,
- satker naive timestamp,
- operasi/skenario,
- marker/building/line/text marker/particle,
- animasi/alur,
- snapshot/last position,
- online user/error.

Lihat [PHASE2_REPORT.md](../../docs/PHASE2_REPORT.md).

## 10. Risiko kontrak yang wajib diuji

1. `units` string versus array.
2. `satker_id` null versus omitted.
3. Timestamp dan timezone.
4. Perbedaan status HTTP 200/201.
5. Error body tanpa field `data`.
6. Pagination 1-based dan nilai `total`.
7. Urutan list berdasarkan `created_at DESC` atau `ASC` per endpoint.
8. Perbedaan akses `user` terhadap data tanpa satker.
9. Public endpoint yang mengekspos data operasi/skenario.
10. Perilaku route admin untuk `admin_monitor`.
