# Golden Response

Folder ini akan berisi respons server Go yang diambil dari instance test lokal.

- `capture-summary.generated.md` — status, path, dan ukuran setiap request.
- `capture-meta.json` — metadata capture.
- `redacted/` — respons yang tokennya sudah diganti `<REDACTED>`.
- `raw/` — respons mentah; **diabaikan Git** karena dapat berisi token test.

## Aturan

- Hanya jalankan `scripts/capture-golden.ps1` terhadap `localhost`/`127.0.0.1`.
- Script menolak host non-local kecuali diberi `-AllowRemote`.
- Jangan pernah mengarahkan capture ke production.
- Jangan commit folder `raw/`.
- Golden response adalah referensi kontrak; perbedaan harus disengaja dan
  didokumentasikan.

## Status

- Capture: **81 respons** pada 2026-11-04.
- Verifikasi: **PASS**.
- Error body 403 tersimpan dan tervalidasi.
- Token pada `redacted/` sudah diganti `<REDACTED>`.
- Raw response diabaikan Git.
- Latency server lokal tersedia di `server-latency.generated.md`.
- WebSocket publish/subscribe tersedia di `websocket-capture.json` dan
  `websocket-capture.generated.md`.
- Channel test: `twg-baseline`; cross-satker publish ditolak 403.
