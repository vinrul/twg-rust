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
| `ping` | Aplikasi-level ping |

Server ke klien:

| `type` | Fungsi |
|---|---|
| `hello` | Konfirmasi koneksi, berisi `connection_id`, `protocol`, `user_id`, `role` |
| `ack` | Konfirmasi subscribe/publish, berisi `sequence` |
| `event` | Command yang diterima semua subscriber skenario |
| `presence` | Jumlah koneksi dan daftar `user_ids` unik |
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

## 9. Pemetaan channel lama ke baru

| Lama (Centrifugo) | Baru (`twg.ws.v1`) |
|---|---|
| Channel global `twg-jagradewata` | `subscribe { skenario_id }` per skenario |
| Filter `satker_id` di klien | Otorisasi satker di server saat subscribe/publish |
| Tidak ada sequence | `sequence` per skenario |
| Tidak ada resume | `last_sequence` + history + `resync` |
| Tidak ada ack | `ack`/`error` dengan `request_id` |
| Tidak ada idempotency | `idempotency_key` per skenario |

Dual-publish (opsional) diaktifkan dengan:

```text
CENTRIFUGO_DUAL_PUBLISH=true
CENTRIFUGO_ADDR=http://127.0.0.1:8000/api
CENTRIFUGO_KEY=<key>
CHANNEL=twg-jagradewata
```

Saat aktif, setiap event yang diterima juga dikirim ke channel Centrifugo lama
dengan bentuk payload yang sama (termasuk `command`, `data`, `marker_id`,
`satker_id`), sehingga client React lama tetap menerima command selama migrasi.
