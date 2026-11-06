# Laporan Fase 8 — Client shell dan UI non-peta

Status: **selesai** untuk alur non-peta desktop dan web. Sisa item kecil dicatat
di bagian [Deviasi dan catatan](#deviasi-dan-catatan).

## 1. Cakupan

Fase 8 menambahkan shell client Bevy + egui di atas fondasi peta Fase 3:

- State layar `Boot`, `Login`, `Workspace`, `MapEditor`, `Preview`.
- Tema design token gelap (warna, spacing, radius, shadow, font) dan widget.
- HTTP client typed dengan queue, refresh single-flight, dan retry 401.
- Transport native (`ureq` + `tungstenite`) dan web (`fetch` + `WebSocket`).
- Penyimpanan sesi native (file) dan web (`localStorage`).
- Layar Login, Dashboard, Operasi, Skenario, Latihan, Pengguna, Satker,
  Online, Snapshot, Profil, dan Preview peta.
- Realtime `twg.ws.v1` beserta status koneksi dan tombol reconnect.

## 2. Struktur

```text
crates/twg-client/src/
  api.rs        ApiClient, queue, refresh single-flight, retry 401, error mapping
  design.rs     design token DARK dan konstanta layout
  realtime.rs   WsConnection, WsTransport, status/backoff, resume sequence
  session.rs    Screen, NavItem, ClientSession, ClientState, role guard
  store.rs      TokenStore, MemoryTokenStore, StoredSession, device id
  native.rs     UreqTransport, TungsteniteTransport, FileTokenStore
  web.rs        FetchTransport, BrowserWsTransport, LocalStorageTokenStore

crates/twg-ui/src/
  screens.rs    shell, 9 layar non-peta, modal, toast
  state.rs      ClientUi: data cache, jobs, aksi API, realtime state
  theme.rs      apply token ke egui Style/Visuals
  widgets.rs    button, input, textarea, select, checkbox, radio, switch,
                card, table, tabs, badge, stat card, dialog, date picker,
                drag-and-drop layer, loading/empty/error
```

Dependency tetap sesuai aturan: `twg-client` tidak memakai Bevy/renderer,
`twg-ui` menjadi satu-satunya crate yang menggabungkan client + egui.

## 3. Alur API

`ApiClient` bersifat transport-agnostic dan poll-based:

1. Aksi UI memanggil method (`login`, `request`, `logout`, ...) dan menerima
   `ApiHandle`.
2. `ClientUi::poll()` memanggil `api.tick()` setiap frame.
3. Request normal diberi `Authorization: Bearer ...` dan `X-Device-ID`.
4. Bila access token kedaluwarsa, request ditahan sementara dan satu refresh
   dijalankan untuk semua penunggu (single-flight).
5. Bila server tetap membalas 401, request diulang tepat satu kali setelah
   refresh berhasil.
6. Gagal refresh membersihkan sesi dan mengembalikan error ke UI.

Rate limit login/refresh dari server dipetakan ke pesan UI tanpa menghentikan
layar lain.

## 4. Transport

### Native

- HTTP: `ureq` 3 di thread terpisah; status non-2xx tetap dikembalikan sebagai
  `HttpResponse` agar envelope error server bisa dibaca.
- WebSocket: `tungstenite` 0.29 dengan satu thread pemilik socket. Socket
  di-set non-blocking setelah handshake; loop yang sama menguras outbound dan
  membaca inbound agar UI tidak pernah memblokir I/O.
- TLS `wss://` memakai `rustls` + webpki roots.

### Web

- HTTP: `fetch` lewat `web-sys` dan `wasm-bindgen-futures`; request berjalan di
  `spawn_local`.
- WebSocket: `WebSocket` browser. Frame outbound dikirim oleh timer 16 ms yang
  membaca queue `WsConnection`, sehingga sistem Bevy tetap sinkron.
- Base URL diambil dari `?api=<origin>` atau origin halaman.

## 5. Penyimpanan sesi

- Native: `FileTokenStore` di `%APPDATA%\twg\session.json` (Windows) atau
  `$XDG_CONFIG_HOME/twg/session.json` / `~/.config/twg/session.json` (Unix).
  Override: `TWG_TOKEN_FILE`.
- Device id disimpan di `device-id` di direktori yang sama.
- Web: `localStorage` key `twg.session` dan `twg.device_id`.

Catatan keamanan: token di `localStorage` rentan terhadap XSS. Build ini tidak
memuat JavaScript aplikasi pihak ketiga dan CSP produksi harus membatasi script;
bila deployment memerlukan hardening tambahan, gunakan cookie HttpOnly di
reverse proxy (di luar scope fase ini).

## 6. Layar dan role

| Layar | Aksi |
|---|---|
| Login | login, device id, error, Enter |
| Dashboard | statistik, aksi cepat, operasi terbaru |
| Operasi | list, search, create, update, delete |
| Skenario | filter operasi, search, create, update, copy, delete |
| Latihan | daftar home operasi |
| Pengguna | list, create, update, delete (admin-like) |
| Satker | list, create, update, delete (admin) |
| Online | user online |
| Snapshot | filter tanggal, muat, toggle, hapus |
| Profil | nama/profile image, ganti password, logout |
| Preview | realtime status, layer order, kembali ke skenario |

Role guard ada di `NavItem::allowed_for` dan `ClientSession`; menu Pengguna
tidak muncul untuk role non-admin. Unit test menutup kasus user biasa dan
`admin_satker`.

## 7. Verifikasi

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p twg-client -p twg-render -p twg-ui -p twg-web \
  --target wasm32-unknown-unknown -- -D warnings
cd apps/web && trunk build
node tools/client-smoke.mjs --username=baselinemonitor
```

Hasil:

- 21 test `twg-client` PASS (termasuk refresh single-flight, retry 401, logout,
  role guard, reconnect backoff, dan round-trip file store).
- Seluruh test workspace PASS; clippy native dan WASM bersih.
- `trunk build` berhasil.
- Smoke Chrome headless: sesi dipulihkan dari `localStorage`, `GET
  /private/identify` 200, dashboard memuat `GET /public/list/operasis` 200,
  pindah ke Peta memicu `POST /private/ws-ticket` 200 dan koneksi
  `ws://127.0.0.1:8010/ws?ticket=...`, tanpa exception JavaScript.
- Tile OSM termuat di zoom 2, 13, 14, 16, 19, 23, dan 26 (overzoom memakai
  tile z19).
- Screenshot: [`phase8-client.png`](assets/phase8-client.png) dan
  [`phase8-client-map.png`](assets/phase8-client-map.png).

Smoke web menemukan bug z-fighting: ground fallback berada 5 cm di bawah bidang
tile sehingga menutupi tile pada beberapa tingkat zoom. Ground sekarang
diturunkan secara proporsional terhadap jarak kamera, dan peta tampil di semua
zoom yang diuji.

## 8. Deviasi dan catatan

Item checklist yang belum sepenuhnya setara rencana awal:

| Item | Status | Catatan |
|---|---|---|
| Keyring desktop | Belum | Memakai file store; `TokenStore` sudah berupa trait sehingga keyring bisa ditambahkan tanpa mengubah UI |
| Upload progress | Belum | Belum ada pemilih file/upload di layar non-peta; endpoint upload server sudah siap dari Fase 6 |
| Pembatalan request | Belum | Tidak ada cancellation token; request yang sudah berjalan diselesaikan |
| 403 tertentu membersihkan sesi | Tidak | 403 diperlakukan sebagai error otorisasi tanpa menghapus sesi, karena role guard UI sudah mencegah aksi ilegal; sesi dihapus saat refresh gagal |
| Sheet | Belum | Dialog modal dan popover/menu tersedia; sheet samping belum dipakai layar mana pun |

## 9. Langkah berikutnya

Fase 9 — map core dan editor: integrasi peta ke layar Preview, editor marker,
line/polygon/building, layer panel, toolbox, undo/redo, geocode, measure, dan
penyimpanan skenario.
