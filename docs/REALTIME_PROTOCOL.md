# Protokol Realtime `twg.ws.v1`

Dokumen ini menetapkan protokol WebSocket v1 untuk Tactical Game 3D.
Protokol memakai JSON teks (bukan protobuf) dan satu koneksi per klien.

## 1. Tiket dan upgrade

Access token tidak pernah dikirim melalui URL. Klien meminta tiket sekali pakai:

```http
POST /private/ws-ticket
Authorization: Bearer <access-token>
X-Device-ID: <device-id>
```

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "ticket": "<64 hex characters>",
    "protocol": "twg.ws.v1",
    "expires_in": 30,
    "ws_path": "/ws"
  }
}
```

Lalu buka:

```text
ws://host/ws?ticket=<ticket>
```

Aturan:

- Tiket berlaku 30 detik (dapat diatur `WS_TICKET_TTL_SECONDS`, 5–300 detik).
- Tiket hanya dapat dipakai sekali. Upgrade kedua dengan tiket yang sama ditolak.
- Saat upgrade, server memvalidasi session user + device. Session yang sudah
  di-revoke ditolak.
- `access_token` dan refresh token tidak boleh muncul di URL, query, atau log.
- Query string tidak dicatat oleh middleware request logging (hanya path).

## 2. Framing pesan

Setiap pesan adalah satu frame teks berisi JSON dengan field `type`.

Klien ke server:

| `type` | Fungsi |
|---|---|
| `subscribe` | Berlangganan skenario, opsional resume dari `last_sequence` |
| `unsubscribe` | Berhenti berlangganan skenario |
| `publish` | Mengirim command |
| `resume` | Berlangganan ulang dan meminta event setelah `last_sequence` |
| `presence` | Meminta jumlah koneksi dan user pada skenario |
| `watch_live` | Berlangganan sesi layar besar dan menerima snapshot awal |
| `unwatch_live` | Berhenti menerima sesi layar besar |
| `ping` | Aplikasi-level ping |

Server ke klien:

| `type` | Fungsi |
|---|---|
| `hello` | Konfirmasi koneksi, berisi `connection_id`, `protocol`, `user_id`, `role` |
| `ack` | Konfirmasi subscribe/publish, berisi `sequence` |
| `event` | Command yang diterima semua subscriber skenario |
| `presence` | Jumlah koneksi dan daftar `user_ids` unik |
| `live` | Snapshot sesi layar besar; dikirim saat watch dan setiap perubahan |
| `resync` | Klien harus memuat ulang state dari snapshot |
| `error` | Kesalahan terstruktur |
| `pong` | Balasan `ping` |

## 3. Subscribe

```json
{
  "type": "subscribe",
  "request_id": "sub-1",
  "skenario_id": "44444444-4444-4444-8444-444444444441",
  "last_sequence": 12
}
```

`last_sequence` opsional. Server membalas `ack` dengan sequence terakhir, lalu:

- Jika history tersedia: event setelah `last_sequence` dikirim berurutan.
- Jika history tidak lagi mencakup sequence tersebut: `resync` dikirim.

Otorisasi memakai satker skenario:

- `admin` dan `admin_monitor`: semua skenario.
- `admin_satker` dan `user_satker`: hanya skenario dengan `satker_id` yang sama.
- `user`: hanya skenario tanpa satker (`satker_id IS NULL`).

Subscribe lintas satker dibalas:

```json
{
  "type": "error",
  "request_id": "sub-1",
  "code": "forbidden",
  "message": "cannot access this scenario",
  "retryable": false
}
```

## 4. Publish command

```json
{
  "type": "publish",
  "request_id": "cmd-1",
  "operasi_id": "33333333-3333-4333-8333-333333333331",
  "skenario_id": "44444444-4444-4444-8444-444444444441",
  "marker_id": "55555555-5555-4555-8555-555555555551",
  "command": "MOVE",
  "data": "{\"x\":3,\"y\":4}",
  "is_save": 1,
  "idempotency_key": "client-uuid-1"
}
```

Aturan:

- `command` di-trim. Command kosong ditolak.
- `RELOAD` selalu dipaksa `is_save = 0`.
- `is_save = 1` menyimpan command ke tabel `commands` dan memperbarui
  `last_maps`/`last_positions` sesuai command (`ZOOM`, `MOVE`, `ROT`).
- `operasi_id`, `skenario_id`, dan `marker_id` harus konsisten. Marker harus
  milik skenario tersebut.
- Otorisasi satker sama dengan subscribe.
- Publish lintas satker ditolak sebelum ada efek samping.

Server membalas `ack` dengan sequence event:

```json
{
  "type": "ack",
  "request_id": "cmd-1",
  "skenario_id": "44444444-4444-4444-8444-444444444441",
  "sequence": 13,
  "duplicate": false
}
```

Event yang diterima subscriber:

```json
{
  "type": "event",
  "sequence": 13,
  "operasi_id": "33333333-3333-4333-8333-333333333331",
  "skenario_id": "44444444-4444-4444-8444-444444444441",
  "marker_id": "55555555-5555-4555-8555-555555555551",
  "command": "MOVE",
  "data": "{\"x\":3,\"y\":4}",
  "is_save": 1,
  "satker_id": "11111111-1111-4111-8111-111111111111"
}
```

Field `satker_id` memungkinkan klien lama mempertahankan filter satker.
Field `sequence` naik monoton per skenario.

## 5. Idempotency dan retry

- `idempotency_key` opsional tetapi disarankan untuk command yang dapat di-retry.
- Key disimpan per skenario, maksimal 1.024 key terakhir.
- Publish ulang dengan key yang sama tidak menyimpan atau menyiarkan event baru:

```json
{
  "type": "ack",
  "request_id": "cmd-2",
  "skenario_id": "44444444-4444-4444-8444-444444444441",
  "sequence": 13,
  "duplicate": true
}
```

- `error.retryable = true` berarti klien boleh mencoba lagi (misalnya database
  sementara tidak tersedia). `retryable = false` berarti payload/izin salah.
- Klien harus memakai `request_id` yang sama saat retry agar dapat mencocokkan
  `ack`/`error`.

## 6. Resume dan snapshot fallback

`resume` identik dengan `subscribe` dengan `last_sequence` wajib:

```json
{
  "type": "resume",
  "request_id": "resume-1",
  "skenario_id": "44444444-4444-4444-8444-444444444441",
  "last_sequence": 120
}
```

Server menyimpan `WS_HISTORY_LIMIT` event terakhir (default 256). Bila gap terlalu
besar:

```json
{
  "type": "resync",
  "request_id": "resume-1",
  "skenario_id": "44444444-4444-4444-8444-444444444441",
  "sequence": 512,
  "reason": "history_gap",
  "snapshot": { "...": "game_session_snapshots atau tidak ada" }
}
```

- Jika snapshot tersedia, klien memuat `map_state`, `markers_state`, dan
  `anims_state` dari snapshot.
- Jika `snapshot` tidak ada, klien harus memuat ulang skenario melalui HTTP
  (`GET /public/...`) lalu melanjutkan dari `sequence`.

## 7. Presence dan heartbeat

Permintaan presence:

```json
{"type":"presence","request_id":"p-1","skenario_id":"..."}
```

Balasan (juga disiarkan saat ada koneksi subscribe/unsubscribe/putus):

```json
{
  "type": "presence",
  "request_id": "p-1",
  "skenario_id": "...",
  "count": 3,
  "user_ids": ["...", "..."]
}
```

`count` menghitung koneksi; `user_ids` unik dan terurut.

Heartbeat:

- Server mengirim WebSocket Ping setiap 25 detik.
- Koneksi tanpa aktivitas selama 75 detik ditutup.
- Klien dapat mengirim `{"type":"ping"}` dan menerima `{"type":"pong"}`.

## 8. Batas dan perilaku koneksi

| Batas | Nilai |
|---|---|
| Ukuran frame/message | 64 KB |
| Antrean keluar per koneksi | 256 pesan |
| History per skenario | `WS_HISTORY_LIMIT` (default 256) |
| TTL tiket | `WS_TICKET_TTL_SECONDS` (default 30 detik) |
| Idle timeout | 75 detik |

- Klien lambat (antrean penuh) diputus agar tidak memblokir fan-out.
- Sequence dan idempotency disimpan di memori proses. Untuk deployment
  multi-instance, keduanya perlu dipindahkan ke store bersama.

## 9. Catatan migrasi

Seluruh client memakai `twg.ws.v1` langsung ke server Rust (`GET /ws`). Jembatan
dual-publish Centrifugo sudah dihapus, channel global lama `twg-jagradewata`
tidak lagi dipakai, dan filter `satker_id` di klien digantikan otorisasi satker
di server saat subscribe/publish.

## 10. Sinkronisasi play dari menu Simulasi

Layar play yang dibuka dari menu Simulasi memakai pesan `play` di koneksi WebSocket
yang sama. Editor Play **tidak** memakai pesan ini.

Klien ke server:

```json
{
  "type": "play",
  "request_id": "ui-play",
  "skenario_id": "...",
  "action": "join",
  "data": ""
}
```

| `action` | Fungsi |
|---|---|
| `join` | Bergabung ke sesi play skenario; server mengirim snapshot terakhir ke pengaju dan meminta snapshot segar dari participant lain |
| `leave` | Keluar dari sesi play; participant terakhir yang keluar menghapus snapshot |
| `state` | Mengirim snapshot state play terbaru (disimpan server dan disiarkan ke participant lain) |
| `sync` | Meminta snapshot terakhir yang tersimpan |
| `started` | Memulai sesi tampilan layar besar untuk skenario ini; hanya `admin` dan `admin_satker` |
| `camera` | Mengirim kamera `{lat,lon,zoom,pitch,bearing}` untuk layar besar; hanya `admin` dan `admin_satker` |
| `reload` | Menandai skenario layar besar perlu dimuat ulang; hanya `admin` dan `admin_satker` |
| `stop` | Mengakhiri sesi tampilan layar besar; hanya `admin` dan `admin_satker` |

Server ke klien (`play_event`):

```json
{
  "type": "play_event",
  "sequence": 0,
  "skenario_id": "...",
  "actor": "...",
  "action": "state",
  "data": "{\"units\":[...],\"routes\":[...]}"
}
```

- `actor` adalah `connection_id` pengirim; client mengabaikan event miliknya
  sendiri. `actor` nil berarti snapshot dari server.
- `action: "sync"` meminta participant play lain mengirim snapshot segar.
- `started` menyiapkan sesi tampilan. Client yang bukan `admin` atau
  `admin_satker` tidak boleh memuat skenario baru di layar besar, tetapi
  `state` dari semua participant tetap memengaruhi animasi dan gerakan yang
  sedang berjalan. Admin/admin_satker mengirim `started` saat membuka skenario
  dari daftar Simulasi, memilih skenario di tab Skenario, atau menekan **Muat
  ulang**; role lain tidak mengirim `started`.
- `camera` tidak disiarkan sebagai `play_event`; nilainya disimpan pada sesi
  publik dan dibaca layar besar.
- Pesan `play_event` hanya dikirim ke koneksi yang sudah `join`, bukan ke semua
  subscriber skenario, sehingga editor tidak menerima noise play.
- Sequence tidak dipakai untuk history/resume; snapshot terakhir adalah sumber
  sinkronisasi untuk client yang bergabung belakangan.
- Snapshot `state` juga dikirim berkala (±250 ms) saat ada unit/rute bergerak,
  sehingga layar besar dan client yang bergabung terlambat tetap melihat posisi
  terbaru.
- Kamera hanya disinkronkan ke layar besar, bukan ke participant play lain.

Payload snapshot (JSON string di field `data`):

```json
{
  "units": [
    {
      "marker_id": "...",
      "lon": 115.225,
      "lat": -8.648,
      "heading": 90.0,
      "altitude": 0.0,
      "target": [115.226, -8.649],
      "limited": false
    }
  ],
  "routes": [
    { "line_id": "...", "state": "playing", "progress_m": 12.5 }
  ],
  "hidden": ["<marker_id>", "<particle_id>"]
}
```

Unit dan rute dicocokkan lewat UUID server (`markers.id` dan `plot_lines.id`),
sehingga client yang sama-sama punya skenario dapat menerapkannya tanpa ID
lokal. Rute multiple marker tetap dikirim sebagai satu entri `routes` (progres
jalur) ditambah satu entri `units` untuk setiap marker yang bergerak, jadi
client tidak perlu menghitung formasinya sendiri. Field `hidden` berisi UUID
marker dan particle yang sedang disembunyikan animasi **spawn/hide**; client
menerapkannya dengan memperbarui daftar objek tersembunyi lalu membangun ulang
scene hanya jika daftar itu berubah. Field ini opsional agar snapshot lama
tetap kompatibel (snapshot tanpa `hidden` tidak mengubah visibilitas). Ukuran
frame tetap dibatasi 64 KB; snapshot dengan unit sangat banyak harus dipangkas.

## 11. Layar besar

Halaman layar besar sekarang memerlukan login dan memakai WebSocket yang sama.
Role khusus `big_screen` hanya dapat membuka halaman ini; role lain juga dapat
membukanya dari Dashboard. Klien berlangganan sesi tampilan dengan:

```json
{"type":"watch_live","request_id":"ui-live-watch"}
```

Server membalas snapshot awal (`session` tidak ada berarti belum ada sesi):

```json
{
  "type": "live",
  "session": {
    "scenario_id": "...",
    "session_version": 3,
    "revision": 0,
    "state": "{\"units\":[],\"routes\":[]}",
    "state_version": 7,
    "camera": "{\"lat\":-2.5,\"lon\":118.0,\"zoom\":5.0,\"pitch\":0.0,\"bearing\":0.0}",
    "camera_version": 2
  }
}
```

Setelah itu server mendorong pesan `live` baru setiap kali `started`, `state`,
`camera`, `reload`, atau `stop` mengubah sesi. Klien tidak perlu polling.

- `session` tidak ada berarti belum ada admin yang memulai simulasi; layar besar
  menampilkan peta Indonesia.
- `session_version` berubah setiap admin memulai ulang sesi; layar besar memuat
  ulang skenario. `revision` disediakan untuk reload sesi yang sama tanpa
  memulai sesi baru; tombol **Muat ulang** saat ini memulai sesi baru sehingga
  `session_version` yang berubah.
- `state_version` dan `camera_version` naik monoton. Layar besar hanya menerapkan
  nilai yang versinya berubah.
- Hanya koneksi terautentikasi yang dapat mengirim `watch_live`; semua role yang
  sudah login boleh menonton. Pesan `started`, `camera`, `reload`, dan `stop`
  tetap hanya diterima dari `admin`/`admin_satker`.
- Manifest aset tetap memakai `GET /private/assets/manifest` karena halaman
  sudah login; berkas model/gambar disajikan dari route statis `/assets`.
- Server hanya menyimpan satu sesi tampilan aktif dalam memori. Memulai sesi baru
  menggantikan sesi sebelumnya; deployment multi-instance memerlukan store
  bersama agar semua instance melihat sesi yang sama.
