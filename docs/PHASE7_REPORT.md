# Laporan Fase 7 — WebSocket dan Real-time

**Tanggal:** 6 November 2026

**Status:** PASS — protokol `twg.ws.v1` selesai, termasuk tiket sekali pakai,
channel per skenario dengan otorisasi satker, sequence, idempotency, ack/error,
presence, heartbeat, resume dari history, resync dengan snapshot fallback, dan
dual-publish Centrifugo opsional. Smoke test end-to-end lulus.

## Protokol

Spesifikasi lengkap ada di [`docs/REALTIME_PROTOCOL.md`](REALTIME_PROTOCOL.md).

Ringkasan:

- `POST /private/ws-ticket` menerbitkan tiket 64 hex, berlaku 30 detik, sekali
  pakai, terikat pada user + device.
- `GET /ws?ticket=...` melakukan upgrade. Access token tidak pernah masuk URL.
- Pesan JSON dengan field `type`: `subscribe`, `unsubscribe`, `publish`,
  `resume`, `presence`, `ping` dari klien; `hello`, `ack`, `event`, `presence`,
  `resync`, `error`, `pong` dari server.
- Sequence monoton per skenario. History default 256 event.
- `idempotency_key` opsional mencegah command ganda disimpan/disiarkan ulang.
- Resume mengirim event yang tertinggal; jika gap melebihi history, server
  mengirim `resync` berisi snapshot bila ada, atau `snapshot` kosong agar klien
  memuat ulang skenario.
- Presence menghitung koneksi dan user unik per skenario.

## Implementasi

- `crates/twg-contracts/src/realtime.rs` — kontrak pesan klien/server dan
  `WsTicketResponse`.
- `crates/twg-server/src/realtime/ticket.rs` — tiket sekali pakai dengan TTL.
- `crates/twg-server/src/realtime/hub.rs` — registry koneksi, subscription,
  sequence, history, idempotency, presence, dan kebijakan slow consumer.
- `crates/twg-server/src/realtime/legacy.rs` — dual-publish ke channel Centrifugo
  lama memakai HTTP API (`publish`).
- `crates/twg-server/src/handlers/realtime.rs` — issuance tiket, upgrade,
  loop baca/tulis, heartbeat, otorisasi, persist command, broadcast, resync.
- `AppState.realtime` — `Arc<Realtime>` siap dipakai handler dan test.
- Konfigurasi baru: `WS_TICKET_TTL_SECONDS`, `WS_HISTORY_LIMIT`,
  `CENTRIFUGO_DUAL_PUBLISH`, `CENTRIFUGO_ADDR`, `CENTRIFUGO_KEY`, `CHANNEL`.

## Keamanan

- Tiket terpisah dari access token, berumur pendek, dan sekali pakai.
- Session divalidasi ulang saat upgrade, sehingga logout/revoke langsung
  memutus kemampuan membuka koneksi baru.
- Subscribe dan publish lintas satker ditolak sebelum efek samping.
- `user` tanpa satker hanya boleh mengakses skenario tanpa satker.
- `marker_id` harus milik `skenario_id`; `operasi_id` harus cocok dengan
  skenario.
- Frame dibatasi 64 KB, antrean keluar 256 pesan; klien lambat diputus.
- Query string (berisi tiket) tidak dicatat oleh request logging.
- Kunci Centrifugo diredaksi dari `Debug` config dan tidak pernah dilog.

## Bukti pengujian

Unit test:

```text
cargo test -p twg-contracts -p twg-server
  twg-contracts: 15 passed
  twg-server: 45 passed
```

Cakupan unit: single-use/expired ticket, sequence per skenario,
subscribe/broadcast/presence, resume dalam history, gap memerlukan resync,
idempotency, slow consumer diputus, unregister membersihkan subscription, dan
serialisasi kontrak protokol.

Smoke test end-to-end (`tools/ws-smoke.mjs`, PostgreSQL + seed baseline):

```text
ws smoke: PASS
```

Skenario smoke:

- Login `admin`, `user_satker`, `user`, dan `admin_monitor`.
- Tiket + upgrade, memastikan access token tidak ada di URL.
- Subscribe skenario satker sendiri berhasil.
- Subscribe lintas satker ditolak `403 forbidden`.
- `user` tanpa satker ditolak pada skenario bersatker.
- `admin_monitor` dapat subscribe lintas satker.
- Publish `MOVE` tersimpan (`is_save=1`) dan diterima subscriber dengan
  `sequence`, `command`, `satker_id`.
- Idempotency: publish ulang dengan key sama menghasilkan `ack.duplicate=true`
  tanpa event kedua.
- `ZOOM is_save=1` memperbarui `last_maps`; `MOVE` memperbarui `last_positions`;
  dibaca ulang lewat HTTP.
- Presence mengembalikan jumlah koneksi dan user.
- `ping`/`pong`.
- Resume dari sequence lama memutar ulang event yang tertinggal.
- Setelah history overflow, resume menghasilkan `resync` dengan snapshot null;
  setelah snapshot disimpan, `resync` membawa snapshot.
- Publish lintas satker ditolak.
- Fan-out 8 klien menerima event dengan sequence yang sama dan urutan benar.
- Reconnect storm 12 koneksi cepat tanpa error.
- Heartbeat Ping setiap 25 detik dan idle timeout 75 detik.

## Keterbatasan dan catatan migrasi

- Sequence, history, dan idempotency disimpan di memori proses. Deployment
  multi-instance memerlukan store bersama (Redis/Postgres) sebelum scale-out.
- Tiket juga disimpan di memori; load balancer harus sticky atau tiket perlu
  ditandatangani untuk multi-instance.
- Dual-publish Centrifugo sudah diimplementasikan, tetapi belum diuji terhadap
  instance Centrifugo produksi pada fase ini. Pemetaan channel lama ke baru
  didokumentasikan di `docs/REALTIME_PROTOCOL.md`.
- Realtime native menggantikan ketergantungan client pada channel global
  Centrifugo; client lama tetap dapat berjalan melalui dual-publish.
