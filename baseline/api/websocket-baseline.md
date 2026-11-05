# Baseline WebSocket / Real-time

## 1. Transport lama

- Client: `centrifuge` JS.
- Server: Centrifugo, publish melalui `gocent` dari Go.
- Channel: **satu channel global** `twg-jagradewata`.
- Endpoint publish: `POST /private/socket`.
- Endpoint WebSocket publik saat ini: `wss://centrifugo.jagradewata.id/connection/websocket`.

## 2. Payload publish

`POST /private/socket` menerima:

```json
{
  "operasi_id": "uuid",
  "skenario_id": "uuid",
  "marker_id": "uuid",
  "command": "MOVE",
  "data": "{\"x\":115.2,\"y\":-8.6,\"z\":0}",
  "is_save": 0
}
```

Server menerbitkan payload:

```json
{
  "operasi_id": "uuid",
  "skenario_id": "uuid",
  "marker_id": "uuid",
  "command": "MOVE",
  "data": "{\"x\":115.2,\"y\":-8.6,\"z\":0}",
  "is_save": 0,
  "satker_id": "uuid-or-omitted"
}
```

`command` di-trim. Command `reload` dipaksa `is_save = 0`. Selain itu
`operasi_id`, `skenario_id`, dan `marker_id` harus UUID valid dan command dapat
disimpan ke tabel `commands` jika `is_save` aktif.

## 3. Command yang diamati

| Command | Peran di client | Data |
|---|---|---|
| `RELOAD` | Muat ulang skenario | biasanya kosong |
| `ZOOM` | Terapkan kamera | JSON center/zoom/pitch/bearing |
| `CHANGE` | Ganti layer/mode | JSON layer |
| `SNAPSHOT_RESTORE` | Pulihkan snapshot | biasanya kosong |
| `MOVE` | Pindahkan marker | JSON posisi/rotasi |
| `ROT` | Rotasi marker | JSON rotasi |
| `MOVEGROUP` | Pindahkan group | JSON group |
| `ROTGROUP` | Rotasi group | JSON group |
| `SHOWINFO` | Tampilkan info marker | — |
| `HIDEINFO` | Sembunyikan info marker | — |
| `SHOWINFO_BUILDING` | Tampilkan info building | — |
| `HIDEINFO_BUILDING` | Sembunyikan info building | — |
| `PLAY_ANIM` | Putar animasi | data animasi |
| `STOP_ANIM` | Hentikan animasi | data animasi |
| `SHOOT` | Efek tembakan | data shoot |
| `SHOW_TEXT_MARKER` | Tampilkan text marker | marker_id/data |
| `HIDE_TEXT_MARKER` | Sembunyikan text marker | marker_id/data |

Catatan: client berbeda memiliki cakupan command yang sedikit berbeda
(`PreviewGame` hanya menangani sebagian command, `MapHome` lebih lengkap).

## 4. Isolasi satker saat ini

- Server memeriksa apakah actor role satker boleh mengirim command ke skenario
  yang memiliki satker berbeda.
- Payload tetap dikirim ke channel global.
- Client memfilter event berdasarkan `satker_id` yang ada di payload.
- Ini bukan isolasi yang kuat: semua subscriber global dapat menerima payload
  dan filter hanya terjadi di client.

## 5. Konsekuensi untuk server Rust

WebSocket Rust harus:

1. Memakai channel/room per satker/operasi/skenario.
2. Menegakkan otorisasi saat subscribe dan publish.
3. Menambahkan sequence per skenario.
4. Menambahkan idempotency key.
5. Menyediakan pesan ack/error.
6. Menyediakan resume setelah reconnect.
7. Tidak mengirim payload sensitif ke client di luar scope.
8. Tidak memakai access token panjang di URL; gunakan ticket sekali pakai.

## 6. Hasil runtime Fase 0

- Centrifugo: `docker.io/centrifugo/centrifugo:v6.2.5` di Podman.
- Config: `baseline/fixtures/centrifugo.json`, `client.insecure=true` untuk test.
- Channel test: `twg-baseline` (terpisah dari channel produksi
  `twg-jagradewata`).
- Koneksi anonim dan subscribe: berhasil.
- Publish lewat `POST /private/socket`: HTTP 201.
- Payload diterima di WebSocket lengkap dengan `operasi_id`, `skenario_id`,
  `marker_id`, `command`, `data`, `is_save`, dan `satker_id`.
- Command `MOVE` dengan `is_save=1` tersimpan di tabel `commands`.
- Publish lintas satker: HTTP 403 `cross-satker command not allowed`.
- Command lintas satker yang ditolak tidak tersimpan.
- Reconnect/resume/snapshot runtime belum diuji.

Bukti: `golden/websocket-capture.json` dan
`golden/websocket-capture.generated.md`.

## 7. Baseline yang masih perlu diambil dari instance lama

- [ ] Nama channel aktual.
- [x] Format pesan Centrifugo (via runtime lokal).
- [ ] Urutan command pada skenario uji.
- [ ] Perilaku reconnect.
- [ ] Perilaku publish gagal saat Centrifugo mati.
- [x] Perilaku `is_save=1` terhadap tabel `commands` (via runtime lokal).
- [ ] Perilaku `reload` terhadap validasi UUID.
- [x] Perilaku command lintas-satker (403 via runtime lokal).
- [ ] Perilaku command untuk data tanpa satker.
