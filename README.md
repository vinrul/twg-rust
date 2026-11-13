# Tactical Game 3D — Rust

Migrasi client dan server Tactical Game 3D ke Rust.

- Server: Axum + Tokio + SQLx + PostgreSQL
- Client: Bevy 0.20 + egui
- Peta: crate `twg-map` dari awal
- Target: Windows, Linux, macOS, dan WebAssembly

Source lama di `E:\tactical_game` bersifat **read-only** dan tidak boleh diubah
dari workspace ini. Baseline, kontrak, fixture, dan hasil capture berada di
[`baseline/`](baseline/README.md).

## Status

Fase 1: workspace, batas crate, server health scaffold, desktop/web scaffold,
dan CI native + WASM.
Fase 2: kontrak domain/DTO modular dan golden contract tests.
Fase 3: spike `twg-map` — tile OSM dan Esri, overzoom sampai zoom 26,
kamera, cursor lat/lon, marker GLTF, overlay FPS, cache tile, dan stress test
1k/5k/10k marker. Native dan Chrome WebGL2 sudah diuji. Hasil web: 9/9 skenario
PASS, minimum 47,19 FPS pada 10.000 marker. Input web sudah stepwise (wheel
Line/Pixel dinormalisasi), pan mengikuti bearing kamera (diuji pada 45°),
marker memakai `GeoAnchor` agar ikut saat peta di-pan, dan tersedia tombol **Full 9.6k / LOD 2.4k** (shortcut `M`). Klik
marker memilih model dengan selection ring; memori native dan web sudah diukur
lewat `--memory-probe` / `?memory=1`.

Fase 3 juga mencakup fly-to (double-click, `N` ke utara), panel kompas/zoom,
quadtree tile cover, placeholder/error tile, tile proxy server-side dengan rate
limit, serta overlay line, polygon/building extrusion, dan floor plan. Detail
teknis dan angka terbaru ada di [laporan Fase 3](docs/PHASE3_REPORT.md).

Fase 4: fondasi server Axum + PostgreSQL — config `app.env`/environment,
request ID, error envelope, CORS allowlist, body limit, readiness DB, graceful
shutdown, rate limit login/refresh, dan migrasi SQLx. Auth memakai bcrypt legacy,
PASETO v2.local, access/refresh token, session per device (single-device untuk
`admin`/`admin_satker`), rotasi refresh single-flight, identify, logout, dan
password change. Parity token diuji dua arah dengan server Go lama. Detail ada di
[laporan Fase 4](docs/PHASE4_REPORT.md).

Fase 5: endpoint read-only — 14 endpoint publik (skenario, marker, text marker,
particle, building, line, alur, anim, last position, home operasi, user online,
snapshot), tujuh endpoint privat dengan satker policy, dan dua endpoint admin
category. Pagination `pageSize`/`pageNumber`, search case-insensitive, mapping
DTO, dan timestamp WITA mengikuti server Go. Differential terhadap server Go
lama pada database baseline yang sama: **193 check PASS, 0 FAIL, 3 skip** yang
tervalidasi golden. Detail ada di [laporan Fase 5](docs/PHASE5_REPORT.md).

Fase 6: write API dan upload — CRUD user, satker, operasi, skenario (termasuk
copy dengan remap id JSON), marker, building, line, text marker, alur, seluruh
tipe animasi, particle/shoot, snapshot, dan command `last_map`/`last_position`.
Operasi multi-tabel memakai transaksi. Upload gambar/video memvalidasi MIME dari
magic bytes dengan `infer`, membatasi ukuran, menyanitasi nama file, dan
menyajikan `/uploads` dengan header aman. Differential write terhadap server Go
lama: **89 check PASS** plus 8 safety check. Detail ada di
[laporan Fase 6](docs/PHASE6_REPORT.md).

Fase 7: realtime `twg.ws.v1` — tiket WebSocket sekali pakai 30 detik, upgrade
`/ws`, channel per skenario dengan otorisasi satker, sequence, idempotency,
ack/error, presence, heartbeat, resume dari history, dan resync dengan snapshot
fallback. Smoke test end-to-end PASS
(termasuk cross-satker, duplicate command, fan-out 8 klien, dan reconnect storm).
Spesifikasi ada di [protokol realtime](docs/REALTIME_PROTOCOL.md), laporan di
[laporan Fase 7](docs/PHASE7_REPORT.md).

Fase 8: client shell Bevy + egui — state layar, tema dan widget, HTTP client
typed dengan refresh single-flight, transport native (`ureq`/`tungstenite`) dan
web (`fetch`/`WebSocket`), penyimpanan sesi, serta layar Login, Dashboard,
Operasi, Skenario, Latihan, Pengguna, Satker, Online, Snapshot, Profil, dan
Preview. Alur non-peta diverifikasi di desktop dan Chrome headless; detail dan
deviasi ada di [laporan Fase 8](docs/PHASE8_REPORT.md).

`cargo check`/`build` WebAssembly dan `trunk build --cargo-profile
wasm-release` sudah berhasil. Bundle WASM release 46 MB; optimasi
`wasm-opt`/Brotli dikerjakan pada fase packaging.

Belum diimplementasikan pada fase ini:

- Editor peta dan authoring line/polygon/floor plan.
- Animasi runtime penuh.
- Upload gambar/video dari client shell.
- Feature parity dengan React/Go.

## Struktur

```text
crates/
  twg-domain/          aturan role dan scoping satker
  twg-contracts/       envelope respons dan kontrak bersama
  twg-auth/            bcrypt, PASETO v2.local, session, dan auth service
  twg-storage/         koneksi PostgreSQL/SQLx, migrasi, auth store, dan read store
  twg-server/          router Axum, auth, middleware, health, dan tile proxy
  twg-client/          state client
  twg-map/             geo, tile source/cover, dan kamera peta
  twg-render/          plugin Bevy, tile OSM, marker, kamera
  twg-ui/              plugin egui dan overlay peta
  twg-asset-pipeline/  pipeline aset (scaffold)
apps/
  server/              binary server
  desktop/              binary native Bevy
  web/                  library WASM + Trunk
  online-admin/         app desktop admin: user online + kick sesi
compose.yaml           PostgreSQL untuk development lokal
tools/
  web-stress.mjs       harness stress browser (Chrome DevTools Protocol)
  client-smoke.mjs     smoke client shell web (sesi, API, WebSocket, screenshot)
  assets-smoke.sh      smoke upload/konversi/manifest asset library
  kick-smoke.sh        smoke daftar online + kick sesi (admin)
  phase4-smoke.mjs     smoke test auth terhadap server + PostgreSQL
  paseto-parity.mjs    parity token Rust vs server Go lama
  read-parity.mjs      differential endpoint read-only Rust vs server Go lama
  write-parity.mjs     differential endpoint write + safety check Rust vs Go
  ws-smoke.mjs         smoke test WebSocket twg.ws.v1 end-to-end
```

Aturan dependency:

- `twg-domain` tidak boleh mengimpor Axum, SQLx, Bevy, atau browser API.
- `twg-contracts` tidak boleh mengimpor renderer.
- `twg-server` tidak boleh mengimpor Bevy.
- App client tidak boleh mengimpor crate server.

## Prasyarat build native

Build native (desktop/server) memerlukan CMake karena `draco_decoder`
mengompilasi Draco dari source, memakai `cc`/`cxx` dan C++ toolchain host:

- CMake 3.16+ — `brew install cmake` (macOS) atau `sudo apt install cmake` (Debian/Ubuntu).
- Header standard library C++ yang lengkap.

Build WASM tidak memerlukan keduanya: untuk `wasm32` `draco_decoder` tidak
menarik `cxx`/`cc` dan `build.rs`-nya langsung berhenti.

### macOS

Dua kondisi berikut membuat `cargo run -p twg-desktop` gagal sebelum satu baris
kode repo ikut dikompilasi:

1. **Deployment target lebih baru dari macOS host.** `rustc` default ke `minos 11.0`
   untuk `aarch64-apple-darwin`, sedangkan `cc` memakai `xcrun --show-sdk-version`
   bila `MACOSX_DEPLOYMENT_TARGET` kosong. Di mesin dengan SDK 27 tapi macOS 26,
   objek C++ dibangun dengan `-mmacosx-version-min=27.0` dan linker memberi warning
   `object file was built for newer macOS version`. Set eksplisit:

   ```sh
   export MACOSX_DEPLOYMENT_TARGET=26.0   # sesuaikan dengan `sw_vers -productVersion`
   ```

2. **Header libc++ tidak lengkap.** Gejalanya
   `fatal error: 'algorithm' file not found` saat kompilasi `cxx` atau
   `draco_decoder`, padahal error-nya bukan dari kode proyek. Cek dengan:

   ```sh
   echo '#include <algorithm>' | c++ -x c++ -fsyntax-only - && echo OK
   ls /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/include/c++/v1/algorithm
   ```

   Bila perintah pertama gagal sementara file di SDK ada, penyebabnya folder sisa
   instalasi lama yang tidak lengkap, misalnya
   `/Library/Developer/CommandLineTools/usr/include/c++/v1` yang hanya berisi
   `__bit`/`cxxabi.h`. Karena folder itu ada, clang memakainya dan tidak menambah
   path SDK. Perbaikan tuntas — install ulang Command Line Tools:

   ```sh
   sudo rm -rf /Library/Developer/CommandLineTools
   sudo xcode-select --install
   ```

   Workaround cepat tanpa install ulang: pindahkan folder sisa tersebut
   (`sudo mv .../usr/include/c++ .../usr/include/c++-stale.bak`), atau set
   `CPLUS_INCLUDE_PATH` ke path libc++ milik SDK.

Setelah memperbaiki header atau memasang CMake, ulangi build dari crate native
agar build script yang sebelumnya gagal/ter-cache dijalankan ulang:

```sh
cargo clean -p cxx -p draco_decoder
```

## Perintah

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p twg-domain -p twg-contracts -p twg-auth -p twg-storage -p twg-map -p twg-client -p twg-server -p twg-render
cargo check -p twg-web --target wasm32-unknown-unknown
```

Server:

```bash
cp .env.example .env   # sesuaikan DB_SOURCE dan TOKEN_SYMMETRIC_KEY
cargo run -p twg-server-app
```

`.env` dibaca dari working directory; environment process tetap menang. File
lama `app.env` masih didukung dan menimpa `.env` bila ada.
`TOKEN_SYMMETRIC_KEY` wajib tepat 32 byte. `readyz` melakukan ping PostgreSQL;
`GET /tiles/...` tetap tersedia untuk proxy tile.

### PostgreSQL lokal (Docker Desktop)

```bash
docker compose up -d
docker compose ps
```

`compose.yaml` menjalankan:

| Service | Container | Address |
|---|---|---|
| PostgreSQL 18 | `twg-db-local` | `127.0.0.1:55432` |

Nilai database diambil dari `.env` (`POSTGRES_USER`, `POSTGRES_PASSWORD`,
`POSTGRES_DB`, `POSTGRES_PORT`). Volume data bernama `twg_twg-db-data`;
reset total dengan `docker compose down -v`.

Setelah server pertama kali berjalan (migrasi SQLx otomatis), isi fixture
baseline:

```bash
docker exec -i twg-db-local psql -U twg_baseline -d twg_baseline \
  -v ON_ERROR_STOP=1 -f - < baseline/fixtures/seed.sql
```

Realtime berjalan langsung di server Rust (`GET /ws`), jadi tidak ada
container tambahan yang perlu dijalankan.

Desktop:

```bash
cargo run -p twg-desktop
```

Desktop membuka client shell dan memakai `TWG_API_URL` (default
`http://127.0.0.1:8010`). Gunakan `--stress ...` atau `TWG_DEMO=1` untuk
menjalankan map demo tanpa shell. Desktop mencari aset di `../../assets`
relatif terhadap direktori kerja; override dengan `TWG_ASSET_PATH`.

Stress marker memakai mesh LOD2 secara default. Tambahkan `--full` untuk mesh
penuh dan `--draco` untuk varian terkompresi (lihat
[laporan Draco](docs/DRACO_REPORT.md)). Di web, padanan query-nya adalah
`?lod=1` dan `?draco=1`. Toggle runtime: `M` untuk LOD, `D` untuk Draco.

### Asset library

Setiap asset punya `name`, satu `category`, source file, dan daftar varian hasil
konversi. Kategori menentukan tipe file yang diterima dan konversi yang
tersedia:

| Category | Upload | Konversi | Varian |
|---|---|---|---|
| `model`, `model_kind=static` | 1 file `.glb`/`.gltf` (field `file`) | Draco, LOD2, Draco+LOD2 | `draco.glb`, `lod2.glb`, `draco_lod2.glb` |
| `model`, `model_kind=animated` | 2 file: `idle` + `animated` (field `idle` dan `animated`) | Draco, LOD2, Draco+LOD2 per slot | sama, satu set per slot |
| `texture` | `.png`, `.jpg`, `.jpeg`, `.webp` (field `file`) | resize | `resize_512.*`, `resize_256.*`, `resize_128.*` |
| `icon` | `.png`, `.jpg`, `.jpeg`, `.webp` (field `file`) | resize | sama dengan texture |

Model animasi wajib mengirim dua file: `idle` dan file `animated` yang benar-
benar punya array `animations` (upload ditolak bila kosong). Konversi pada model
animasi dijalankan per slot, misalnya `{"target":"draco","slot":"animated"}`.

Layout file: `<ASSETS_DIR>/<asset-id>/<slot>/<file>`, misalnya
`<id>/idle/original.glb`, `<id>/idle/draco.glb`, `<id>/animated/draco_lod2.glb`.

Deteksi saat upload (per source):

- **Draco** dari `extensionsUsed: KHR_draco_mesh_compression` (atau penanda
  `extras.twg.draco` yang ditulis server).
- **LOD** dari `extras.twg.lod` atau penanda nama file (`..._lod2.glb`,
  `...-lod.glb`). Nama file tanpa angka dianggap LOD1.

Bila file unggahan sudah memenuhi target, konversi tidak dijalankan dan varian
menunjuk ke source file (`reused: true`) — misalnya upload yang sudah Draco+LOD2
lalu menekan convert Draco+LOD2. Setiap perubahan metadata atau varian menaikkan
`version`, sehingga client bisa mendeteksi asset yang perlu disinkronkan. Satu
pengecualian: **rename** tidak menaikkan `version`, karena file tidak berubah dan
desktop tidak perlu mengunduh ulang asset besar hanya karena namanya diganti.

Endpoint (unggah/ubah/hapus hanya role `admin`; baca untuk semua sesi login):

| Method | Path | Keterangan |
|---|---|---|
| POST | `/admin/assets` | multipart `name`, `category`; model juga `model_kind` (`static`/`animated`), lalu `file` atau `idle`+`animated` |
| PUT | `/admin/assets/{id}` | `{"name", "category"}` (kategori hanya bisa berpindah antara texture/icon) |
| POST | `/admin/assets/{id}/source` | multipart `slot` (`static`/`idle`/`animated`) + `file`: ganti file satu slot pada asset yang sudah ada; varian slot itu dihapus dan `version` naik |
| DELETE | `/admin/assets/{id}` | hapus baris + file |
| POST | `/admin/assets/{id}/convert` | `{"target":"draco"}`, `{"target":"lod2"}`, `{"target":"draco_lod2"}`, `{"target":"resize","size":512}`; tambahkan `"slot"` untuk model animasi dan `force: true` untuk konversi ulang |
| GET | `/private/assets` | daftar, filter `?category=` dan `?search=` |
| GET | `/private/assets/manifest` | manifest ringkas untuk sinkronisasi |
| GET | `/private/assets/{id}` | detail satu asset |
| GET | `/assets/{id}/{slot}/{file}` | source/varian file |

Konversi glTF memakai `gltf-transform` (`npm i -g @gltf-transform/cli`) dan
membutuhkan CMake untuk build `draco_decoder` (lihat
[prasyarat build native](#prasyarat-build-native)). `ASSET_LOD2_RATIO` dan
`ASSET_LOD2_ERROR` mengatur simplifikasi; resize gambar dikerjakan di server
memakai crate `image`. File disimpan di `ASSETS_DIR` (default `asset-store`)
dengan satu direktori per id asset.

Upload dan hasil konversi dinormalisasi: bila gambar di dalam GLB sebenarnya
PNG/WebP/KTX2 tetapi `mimeType`-nya salah (misalnya `image/jpeg`), server
memperbaiki field itu dari magic bytes. Untuk asset lama yang diunggah sebelum
perbaikan ini, jalankan sekali dari root project:

```sh
cargo run -p twg-server-app -- --repair-assets
# atau tanpa menimpa binary server yang sedang berjalan:
cargo run -p twg-server --example repair_assets -- asset-store
```

Perintah ini memperbaiki file GLB, memperbarui `size_bytes`/`sha256` di database,
menaikkan `version` asset, dan membuat client menyinkronkan ulang cache offline.

### Aset di desktop (preview server + cache offline)

Menu **Aset** tersedia di `twg-desktop` untuk semua role; unggah, edit, konversi,
dan hapus hanya muncul untuk role `admin`.

- **Upload**: tarik & lepas file ke jendela aplikasi. Pilih kategori (Model /
  Texture / Icon) dan, untuk model, `Static` (1 file) atau `Animasi` (2 file:
  `idle` + file animasi). Nama terisi otomatis dari nama file.
- **Edit nama**: ubah isian **Nama aset** di panel detail lalu tekan **Simpan
  nama** (atau Enter). Tombol **Batal** mengembalikan isian ke nama saat ini.
- **Ganti file**: tombol **Ganti file** pada kartu source (mis. `Idle`, `Animasi`,
  `Static`) — setelah itu tarik file baru ke jendela aplikasi. File lama beserta
  varian slot tersebut diganti, `version` naik, dan preview otomatis dimuat
  ulang. Tombol **Batal** membatalkan penantian file. Id asset tidak berubah,
  jadi skenario/editor yang memakai asset ini tetap valid.
- **Preview**: file preview selalu diambil dari **server**, bukan dari cache
  offline. Texture/icon dimuat lewat asset HTTP Bevy; model dirender sebagai
  preview 3D offscreen dari URL server (auto-fit ke bounding box, layer render
  sendiri), lengkap dengan source, varian, ukuran, serta status Draco/LOD.
  Model animasi menampilkan dua preview — **Idle** dan **Animasi** — dan klip
  animasi pertama diputar berulang otomatis. Preview model bisa diputar dengan
  drag mouse; saat tidak di-drag model berputar perlahan secara otomatis.
  Preview mengikuti opsi "Gunakan hasil convert" bila varian tersedia.
- **Konversi**: tombol Draco / LOD2 / Draco+LOD2 per slot (model) dan
  Resize 512/256/128 (gambar). Target yang sudah terpenuhi ditandai `✓` dan
  tidak dikonversi ulang.
- **Gunakan**: menyimpan pilihan ke resource `ActiveAsset` dengan path file dari
  cache offline — varian `draco_lod2 → lod2 → draco` bila opsi "Gunakan hasil
  convert" aktif, kalau tidak file asli. Model animasi memakai pasangan
  `idle` + `animated`.

Sinkronisasi offline:

- Saat `twg-desktop` dijalankan dengan sesi tersimpan (atau setelah login),
  desktop langsung memanggil `/private/assets/manifest` di latar belakang.
  Tambahan/perubahan aset diunduh ke cache offline sebelum editor/game dipakai.
- Setiap kali akan membuka **Peta**, manifest dicek lagi (overlay "Sinkronisasi
  aset") supaya cache tidak stale.
- Hanya file yang `version`-nya berubah atau belum ada yang diunduh; setiap file
  diverifikasi dengan `sha256`, ditulis atomik, dan `manifest.json` baru ditulis
  setelah semua file sukses.
- Bila server tidak terjangkau: aplikasi memakai snapshot cache terakhir dan
  menampilkan peringatan mode offline. Bila cache masih kosong, navigasi ke peta
  dibatalkan agar tidak memakai aset yang belum pasti.
- Lokasi cache: `TWG_ASSET_CACHE` (default `~/Library/Caches/twg/assets` di macOS,
  `%LOCALAPPDATA%\twg\assets` di Windows, `$XDG_CACHE_HOME/twg/assets` di Linux).
- Build web tidak memiliki cache offline; katalog tetap bisa dilihat,
  sedangkan tombol **Gunakan** dan preview server hanya aktif di desktop.

Cache offline hanya dipakai oleh editor/game/latihan/simulasi. Layar **Aset**
selalu membaca file preview dari server supaya admin melihat versi terbaru,
kendati file tersebut sudah ada di cache.

**Pemakaian di runtime**

- Desktop mendaftarkan folder cache sebagai asset source Bevy `twg-cache://`
  sebelum `DefaultPlugins`, sehingga file offline dimuat tanpa menyentuh server.
- Saat **Gunakan** ditekan, `ActiveAsset` dipetakan ke `MarkerModelOverride`:
  slot `static`/`idle` menjadi model `still`, slot `animated` menjadi model
  animasi, dengan flag Draco per file (validasi glTF dimatikan untuk file
  Draco). Marker dan frame animasi langsung dimuat ulang dari file tersebut.
- `TWG_ACTIVE_ASSET=<uuid>` menjalankan aplikasi dengan aset tertentu yang
  sudah ada di cache (berguna untuk memulai simulasi tanpa memilih manual).
- Tombol **Lepas** menghapus pilihan dan mengembalikan model bawaan.
- Editor peta memakai **tab Assets** untuk daftar model server: nama diambil dari
  server, file dari cache offline. Tab **Katalog lokal** sudah dihapus — semua
  model 3D sekarang ditambahkan lewat Assets.

### Online admin (desktop)

App terpisah untuk memantau user yang sedang login dan mencabut sesinya supaya
bisa login lagi (server memakai sesi single-device untuk `admin`/`admin_satker`).

```bash
cargo run -p twg-online-admin
```

- Login memakai `POST /auth/login/admin`: khusus role `admin`, membuat sesi
  multi-device sehingga **tidak** terkena aturan single-device, dan berbagi rate
  limit dengan `/auth/login`. Daftar online diambil dari
  `GET /private/users/online` dan diperbarui otomatis tiap 5 detik (bisa
  dimatikan).
- Tombol **Kick** meminta konfirmasi lalu memanggil
  `POST /admin/users/{id}/kick-session`; semua sesi aktif user tersebut dicabut
  dan user dapat login lagi. Setiap kick dicatat di log server (actor, target,
  jumlah sesi).
- Route lama `POST /public/users/{id}/kick-session` (tanpa autentikasi, temuan
  S1 di `baseline/security/findings.md`) sudah dihapus.
- Konfigurasi: `TWG_API_URL` (default `http://127.0.0.1:8010`),
  `TWG_ADMIN_DEVICE_ID` (opsional, device id tetap), dan
  `TWG_ADMIN_SESSION_FILE` untuk lokasi file sesi (default
  `online-admin-session.json`, terpisah dari sesi `twg-desktop`).
- Route login biasa (`POST /auth/login`) tetap menolak admin yang sudah aktif di
  device lain; hanya route admin-tool yang membuat sesi tambahan.

### Editor peta

Menu **Peta** membuka editor bergaya Godot/Unity: menu bar, toolbar, dock
Scene/Layers, viewport, Inspector, Assets/Console, dan status bar. `Q/W/E/R`
memilih Select/Move/Rotate/Scale, `1..4` memilih Bentuk/Line/Text/Measure,
`Ctrl+S` menyimpan, `Ctrl+Z` undo, dan `Ctrl+D` menduplikasi objek. Marker 3D
ditambahkan dari tab **Assets** (tombol **Tambah** atau drag ke peta); tool
Marker terpisah sudah dihapus. Klik baris di panel **Scene → Layers** akan
memilih objek sekaligus menerbangkan kamera (pan + zoom) ke objek tersebut,
sehingga marker/model 3D langsung terlihat. Mode **Play** menyembunyikan dock.
Di web, `?scenario=<uuid>` membuka skenario tersebut langsung di editor. Detail
ada di [laporan Fase 9](docs/PHASE9_REPORT.md).

**Tab Assets (model server)**

Panel kiri punya dua tab: **Scene** dan **Assets**; dock bawah hanya berisi
**Console**. Tab **Assets** menampilkan model 3D (`glb`/`gltf`) dan tekstur
gambar dari server:

- Datanya dari `GET /private/assets` (kategori `model`); file model dibaca dari
  cache offline (`twg-cache://<asset-id>/<slot>/original.glb`), bukan diunduh
  ulang dari server. Kartu menampilkan nama, jenis (Static/Animasi), dan status
  `siap offline` / `belum diunduh`; ada tombol **Sinkronkan** bila cache belum
  lengkap.
- **Tambah** menaruh marker di tengah peta. Model juga bisa **ditarik dari kartu
  ke viewport**; posisinya diambil dari titik tanah di bawah kursor (mengikuti
  snap grid). Selama drag, peta tidak ikut ter-klik.
- Model dimuat sebagai **scene glTF utuh**, jadi material dan tekstur bawaan file
  tetap dipakai (bukan warna solid). Rotasi X dan altitude default **0**; ukurannya
  dinormalisasi dari batas `POSITION` di file glTF (target tinggi 2 m) lalu bisa
  diubah lewat Scale.
- Marker dari aset menyimpan UUID aset di `markers.unit_id` (kolom yang sama
  dengan katalog unit), jadi setelah **Simpan** dan **Muat ulang** model yang
  sama dimuat lagi dari cache. Bila aset sudah dihapus/belum tersinkron, marker
  kembali ke model katalog pertama sehingga tetap terlihat.
- Model aset otomatis dinormalisasi tingginya (≈2 m) saat mesh selesai dimuat,
  karena satuan ekspor GLB tidak diketahui; ukuran akhir tetap bisa diatur lewat
  Scale di Inspector.
- Model aset otomatis dinormalisasi tingginya (≈2 m) dari batas mesh di file
  glTF, karena satuan ekspor GLB tidak diketahui; ukuran akhir tetap bisa diatur
  lewat Scale di Inspector.
- Inspector marker punya dropdown model dari daftar *Aset server*. Perubahan
  model disimpan lewat `PUT /private/markers/unit/{id}`.

**Bentuk 3D (primitive & custom mesh)**

Tool **Bentuk** (`1`) membuat objek 3D di titik klik. Dropdown **Bentuk** di
toolbar memilih jenisnya: **Kubus/Kotak**, **Silinder**, **Bola**, **Circle**,
**Ground**, dan **Custom mesh**. Semua bentuk bisa dipindah, diputar, dan
diskalakan lewat gizmo Move/Rotate/Scale atau field Transform di Inspector.

- **Custom mesh**: titik footprint digambar dengan klik berurutan (Enter/klik
ganda untuk selesai). Bentuknya bisa diedit lagi dari Inspector — ubah offset
East/North tiap titik, tambah titik, atau hapus titik (minimal 3).
- **Material**: pilih **Solid color** dengan color picker, atau **Texture** yang
diambil dari aset gambar (kategori Texture/Icon). Tekstur diunggah lewat menu
**Aset**; bila file belum ada di cache, editor memakai URL server, dan bila
tidak tersedia sama sekali material kembali ke warna solid.
- Objek bentuk disimpan sebagai record `buildings` dengan metadata bentuk,
rotasi, skala, dan tekstur di kolom `keterangan`, sehingga tetap dimuat ulang
dengan bentuk yang sama.

**Toolbar dan gizmo transform**

Tombol **Select / Move / Rotate / Scale** berupa ikon (tooltip berisi nama dan
shortcut) dan disorot dengan warna aksen saat aktif. Gizmo di peta mengikuti
tool yang dipilih:

- **Select**: tanpa gizmo — objek terpilih ditandai cincin kuning dan bisa
digeser langsung dengan drag. Gizmo transform baru muncul saat memilih
Move/Rotate/Scale, jadi tidak lagi tertukar dengan Select.
- **Move**: tiga panah X/Y/Z (X/Z menggeser di permukaan, Y mengubah
  ketinggian).
- **Rotate**: tiga cincin X/Y/Z; drag cincin untuk memutar pada sumbu itu.
- **Scale**: handle kubus X/Y/Z untuk skala per sumbu, plus **ring kecil di
  tengah** untuk skala seragam (X, Y, Z sekaligus; drag ke kanan/atas
  memperbesar).

Arahkan kursor ke handle sampai tersorot, lalu drag. Perpindahan tool lewat
toolbar atau `Q/W/E/R` langsung membangun ulang gizmo tanpa menunggu perubahan
lain.

Field angka di Inspector (Lon, Lat, Alt, Rot X/Y/Z, Scale, Lebar, Alpha,
Tinggi, Font, Jumlah) bisa **diketik**, **di-drag**, atau diubah dengan tombol
**▲/▼** di sebelahnya (tooltip menunjukkan besar langkahnya, mis. `+1°` untuk
rotasi). Scale memakai tiga desimal dan bisa diperkecil sampai `0.001` (batas
minimum yang sama berlaku untuk gizmo Scale).

Input peta hanya aktif di dalam viewport: roda mouse (zoom) dan drag tombol
tengah/kanan diabaikan saat kursor berada di atas panel, dock, atau panel
Inspector, sehingga scrollbar panel bisa di-scroll tanpa ikut men-zoom peta.

**Tampilan kamera skenario**

Inspector (saat tidak ada objek terpilih) punya bagian **Tampilan kamera** yang
menampilkan pusat (lat/lon), zoom, bearing, pitch, dan base layer aktif. Tombol
**Simpan tampilan** (khusus admin) menyimpan nilai tersebut ke skenario lewat
`PUT /admin/skenarios/{id}`, sehingga reload editor, Play, simulasi, dan latihan
membuka kamera yang sama. Status berubah menjadi `Tersimpan HH:MM:SS` selama
kamera tidak berubah dan kembali `Belum disimpan` setelah kamera digeser.
Skenario indoor/floor plan mempertahankan base layer server-nya.

**Mode tampilan model**

Dropdown **Model** di toolbar mengatur shading model glTF di viewport editor:
**Texture** (default), **Solid**, dan **Wireframe**. Mode ini hanya berlaku di
editor — **Play** (dan game/simulasi/taktikal) selalu menampilkan tekstur, dan
marker dari katalog lokal tetap memakai warna kategori.

**Mode Play (RTS)**

Tombol **▶ Play** menjalankan skenario sebagai simulasi RTS sederhana di
viewport editor:

- Label objek disembunyikan. Kamera tetap bisa di-orbit (klik kanan tahan) dan
  di-pan (tombol tengah) seperti biasa.
- Marker dengan aset model **Animasi** menjadi unit yang bisa dipilih dan
  digerakkan. Marker statis — termasuk aset static berbentuk kubus, silinder,
  bola, atau cone — menjadi penghalang dan tidak bisa dipilih.
- Klik unit untuk memilih, **Shift/Ctrl+klik** menambah/mengurangi pilihan, dan
  seret di tanah kosong untuk memilih banyak unit (marquee). Unit terpilih
  ditandai ring hijau yang mengikuti bidang tanah/peta (bukan lingkaran layar)
  seperti game RTS.
- **Klik kanan** di tanah memerintahkan unit bergerak. Bila beberapa unit
  dipilih, mereka otomatis membentuk formasi grid di sekitar titik tujuan.
- Unit memutar arah mengikuti gerakan dan memutar klip animasi (file
  `animated`) hanya selagi bergerak. Saat berhenti atau sampai tujuan, model
  kembali ke tampilan **idle** (file `idle`), bukan animasi yang di-pause.
  Hanya satu scene yang di-instansiasi per unit: scene idle diganti scene
  animasi saat mulai bergerak, lalu diganti balik saat berhenti. Scene lama
  masuk ke pool terbatas untuk dipakai ulang, dan pembuatan scene baru dibatasi
  per frame agar perubahan status banyak unit sekaligus tidak menyebabkan
  hitch.
- Unit yang akan melewati marker statis berbelok menghindar, lalu kembali
  menuju titik tujuan setelah penghalang terlewati.
- Posisi selama Play tidak mengubah posisi marker di skenario. **Stop**
  mengembalikan editor ke tampilan authoring semula.

Web:

```bash
cargo install trunk --locked
cd apps/web
trunk build
trunk serve
```

Web memakai origin halaman sebagai API default; tambahkan `?api=<origin>` untuk
API lain. Parameter `?stress=`, `?benchmark=`, atau `?memory=` tetap menjalankan
map demo untuk harness Gate G1.

Stress test web otomatis memakai Chrome DevTools Protocol tanpa dependency npm:

```bash
cd apps/web
trunk build --cargo-profile wasm-release
cd ../..
node tools/web-stress.mjs --sample=10
```

Smoke editor web (membuat marker/line/building/text, menyimpan, reload,
menghapus, lalu membersihkan data uji):

```bash
node tools/client-smoke.mjs --username=baselinemonitor \
  --scenario=44444444-4444-4444-8444-444444444441 --editor-test=true
```

`HTTP_SERVER_ADDRESS` dapat dioverride; default `127.0.0.1:8010`.

Tile proxy server-side tersedia di `GET /tiles/{layer}/{z}/{x}/{y}.png`
(`layer`: `osm` atau `satellite`) dengan disk cache di `.tile-cache/`. Client
web memakai route yang sama lewat `?proxy=<origin>`; desktop memakai
`TWG_TILE_PROXY=<origin>`. Bila tidak diset, client mengambil tile langsung
dari provider.

Realtime memakai `POST /private/ws-ticket` lalu `GET /ws?ticket=...`.
`WS_TICKET_TTL_SECONDS` mengatur umur tiket (default 30 detik) dan
`WS_HISTORY_LIMIT` mengatur jumlah event yang disimpan untuk resume (default
256). Detail protokol ada di [protokol realtime](docs/REALTIME_PROTOCOL.md).

## Dependency policy

- Versi dependency di-pin melalui `Cargo.lock`.
- `cargo deny check` menggunakan [`deny.toml`](deny.toml).
- `cargo audit` dijalankan di CI.
- MapTiler tidak dipakai. Peta memakai OSM tanpa API key.

## Dokumen

- [Rekomendasi arsitektur dan library](docs/RUST_MIGRATION_LIBRARY_RECOMMENDATIONS.md)
- [Checklist implementasi](docs/RUST_IMPLEMENTATION_CHECKLIST.md)
- [Laporan Fase 1](docs/PHASE1_REPORT.md)
- [Laporan Fase 2](docs/PHASE2_REPORT.md)
- [Laporan Fase 3](docs/PHASE3_REPORT.md)
- [Laporan stress test web Gate G1](docs/WEB_STRESS_REPORT.md)
- [Laporan Draco + LOD](docs/DRACO_REPORT.md)
- [Laporan Fase 4](docs/PHASE4_REPORT.md)
- [Laporan Fase 5](docs/PHASE5_REPORT.md)
- [Laporan Fase 6](docs/PHASE6_REPORT.md)
- [Laporan Fase 7](docs/PHASE7_REPORT.md)
- [Laporan Fase 8](docs/PHASE8_REPORT.md)
- [Laporan Fase 9](docs/PHASE9_REPORT.md)
- [Protokol realtime](docs/REALTIME_PROTOCOL.md)
- [Baseline Fase 0](baseline/phase0-report.md)
- [Golden HTTP](baseline/golden/verification.generated.md)
- [WebSocket baseline](baseline/golden/websocket-capture.generated.md)
