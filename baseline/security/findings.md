# Temuan Keamanan Baseline

Temuan ini berasal dari inspeksi source read-only. Belum ada pengujian exploit
terhadap instance berjalan. Prioritaskan review sebelum migrasi.

## Kritis

### S1 — Endpoint publik dapat mencabut seluruh sesi user

- Route: `POST /public/users/:id/kick-session`
- Source: `server/api/routes.go:39`, `server/api/admin_sessions.go:10`
- Handler memanggil `RevokeAllActiveSessionsByUser` tanpa autentikasi, role, atau
  CSRF.
- Dampak: siapa pun yang mengetahui/menebak UUID user dapat memaksa logout semua
  perangkat user tersebut.
- Tindakan: pindahkan ke route admin terautentikasi, batasi role, tambahkan audit
  log, dan uji IDOR.

### S2 — Data sesi dan user terekspos melalui endpoint publik

- Route: `GET /public/users/online`
- Source: `server/api/routes.go:38`, `server/api/public_sessions.go:22`
- Respons memuat nama, username, role, profile image, jumlah sesi aktif, waktu
  terakhir dipakai, dan daftar sesi termasuk `device_id`.
- Dampak: enumerasi user, device ID, dan pola aktivitas tanpa login.
- Tindakan: pindahkan ke endpoint admin/private atau minimalkan field dan
  tambahkan otorisasi.
- **Terkonfirmasi golden:** respons 200 berisi 5 user dan detail sesi lengkap.

### S3 — Dead code MapTiler masih menyimpan API key

- Source: `3dmap/src/helpers/maps.ts` fungsi `createMapTiler`.
- Fungsi ini **tidak dipakai**; satu-satunya referensi lain adalah komentar di
  `PreviewGame.tsx`. Aplikasi memakai tile OSM (dan layer satelit Esri), sehingga
  tidak membutuhkan API key.
- Dampak: key tidak aktif, tetapi tetap merupakan kredensial yang tidak perlu
  dan berpotensi disalahgunakan jika fungsi diaktifkan kembali.
- Tindakan: hapus fungsi dan key dari source Rust baru. Rotasi hanya diperlukan
  jika key pernah dipakai di deployment publik.
- Bukan blocker untuk migrasi OSM-only.

## Tinggi

### S4 — Access dan refresh token disimpan di localStorage

- Source: `3dmap/src/context/AuthProvider.tsx:100-101`,
  `3dmap/src/utils/axiosInstance.ts:25-26,44,79-80,93`.
- Dampak: token dapat dibaca oleh XSS atau ekstensi browser.
- Tindakan: untuk web gunakan cookie `HttpOnly`/`Secure`/`SameSite` dengan
  proteksi CSRF; untuk desktop gunakan OS credential store.

### S5 — Real-time memakai channel global dan filter satker di client

- Source: `3dmap/src/utils/constants.ts:44`,
  `3dmap/src/pages/home/MapHome.tsx:5626-5641`,
  `3dmap/src/pages/game/PreviewGame.tsx:4371-4384`.
- Server menerbitkan payload ke satu channel global.
- Dampak: subscriber lain dapat menerima payload command lintas satker; filter
  client bukan kontrol keamanan.
- Tindakan: channel per scope, otorisasi subscribe/publish di server, dan uji
  negatif cross-satker.

### S6 — Endpoint publik mengekspos data operasi dan skenario

- Route publik mencakup daftar operasi, skenario, marker, building, line, anim,
  particle, text marker, last position, dan snapshot.
- Dampak: data operasi dapat diakses tanpa autentikasi jika endpoint memang
  terbuka di deployment.
- Tindakan: konfirmasi apakah ini kebutuhan produk; jika tidak, pindahkan ke
  private. Jika ya, terapkan pembatasan data, rate limit, dan audit.

### S7 — Upload hanya memvalidasi header Content-Type dan memakai nama file asli

- Source: `server/api/uploads.go:36-44,61-68`.
- Tidak ada content sniffing; header dapat dipalsukan.
- Nama file asli digabung ke path upload tanpa sanitasi eksplisit.
- Batas body default 100 MiB dan contoh konfigurasi 1 GiB.
- Tindakan: validasi isi file, generate nama server-side, sanitasi, batasi tipe
  dan ukuran, simpan di luar web root, dan scan bila diperlukan.

### S8 — Tidak ada rate limit pada login dan endpoint publik

- Tidak ditemukan middleware rate limit di `server/api/server.go`.
- Dampak: brute force, enumerasi, dan penyalahgunaan endpoint publik.
- Tindakan: rate limit per IP/user/device, lockout login, dan monitoring.

### S13 — Role `user` dapat membaca daftar seluruh user

- Route: `GET /private/users`
- Source: `server/api/routes.go:153`, grup private tanpa role middleware.
- **Terkonfirmasi golden:** user biasa mendapat 200 dan melihat 5 user lain,
  termasuk role, username, units, dan satker.
- Dampak: enumerasi user internal.
- Tindakan: batasi ke role admin-like, filter data, atau hapus dari client user.

## Sedang

### S9 — URL produksi dan konfigurasi hardcoded di client

- Source: `3dmap/src/utils/apiPaths.ts:1-8`.
- API dan Centrifugo production ditanam langsung; tidak ada environment switch.
- Dampak: risiko salah environment, capture terhadap produksi, dan sulit rotasi.
- Tindakan: konfigurasi berbasis environment dan tolak capture non-local secara
  default.

### S10 — Role `admin_monitor` masuk ke grup route admin

- Grup `/admin` mengizinkan `admin_monitor`; sebagian handler mungkin membatasi
  sendiri, sebagian tidak.
- Dampak: perilaku read-only tidak konsisten dan rawan eskalasi.
- Tindakan: inventarisasi setiap handler dan tegakkan policy terpusat.

### S11 — Log request dapat memuat informasi sensitif

- Gin logger mencatat path dan request ID, bukan body/token.
- Pastikan logger Rust tidak menambahkan header Authorization, query token, atau
  body login ke log.

### S12 — Perbedaan lingkungan waktu

- Server mengubah timestamp ke `Asia/Makassar`.
- Dampak: salah tafsir waktu pada client lintas zona.
- Tindakan: pertahankan RFC3339 dengan offset dan uji golden response.

### S14 — Login HTTP 201 tetapi body `code` 200

- Route: `POST /auth/login`
- Source: `server/api/users.go:281` memakai `http.StatusCreated` dengan
  `successSingle` yang menulis `code: 200`.
- **Terkonfirmasi golden:** status HTTP 201, body `code` 200.
- Dampak: client yang memeriksa `code` dan status HTTP dapat berbeda persepsi.
- Tindakan: pertahankan untuk kompatibilitas atau normalkan pada API versi baru.

## Rendah / catatan

- Password memakai bcrypt cost default; pertimbangkan Argon2id untuk hash baru.
- `pasetors` belum diaudit pihak ketiga; review dan rotasi kunci.
- `BackendUser.units` berupa string JSON; validasi dan batasi ukuran.
- CORS mengizinkan credentials; pastikan tidak ada origin wildcard di produksi.
- `X-Request-ID` diterima dari client tanpa validasi; batasi format/panjang.

## Status tindakan

| ID | Status |
|---|---|
| S1 | Belum diperbaiki; wajib sebelum cutover |
| S2 | Belum diperbaiki; wajib sebelum cutover |
| S3 | Dead code; hapus fungsi/key, bukan blocker OSM |
| S4 | Akan diperbaiki di client Rust |
| S5 | Akan diperbaiki di WebSocket Rust |
| S6 | Perlu keputusan produk |
| S7 | Akan diperbaiki di server Rust |
| S8 | Akan diperbaiki di server Rust |
| S13 | Akan diperbaiki di server Rust |
| S14 | Perlu keputusan kompatibilitas API |
| S9 | Akan diperbaiki di client Rust |
| S10 | Perlu inventarisasi policy |
| S11 | Akan diterapkan di tracing Rust |
| S12 | Akan diuji di contract test |
