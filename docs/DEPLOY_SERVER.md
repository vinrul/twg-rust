# Deploy `twg-server-app` ke VPS (akses langsung port 8122)

Panduan ini memasang **`twg-server-app`** (backend Rust/Axum) di satu VPS Linux
dan membukanya **langsung di port 8122 tanpa Nginx/reverse proxy**. Semua contoh
memakai Ubuntu 24.04, IP `203.0.113.10`, dan database `twg`. Ganti nilai
tersebut sesuai server kamu.

> **Penting — tanpa TLS.** Server hanya melayani HTTP. Password, token, dan isi
> skenario dikirim **tanpa enkripsi**. Cocok untuk jaringan internal/VPN atau
> tahap uji. Untuk internet publik, batasi port 8122 ke IP tertentu, pakai VPN,
> atau pasang TLS di depan server (lihat [Lampiran A](#lampiran-a-nginx--tls-opsional)).
>
> **Web statis tidak dilayani server.** Aplikasi `twg-web` harus di-host
> terpisah (atau diakses lewat desktop). Lihat
> [Web statis](#8-web-statis-opsional).

> Ringkas: satu VPS cukup untuk satu instance server. Server menyimpan rate
> limit, tiket WebSocket, riwayat realtime, dan idempotency **di memori proses**,
> jadi jangan menjalankan dua instance di belakang load balancer tanpa sticky
> session.

## Arsitektur

```mermaid
graph TD
    D[twg-desktop] -->|HTTP :8122| S[twg-server-app 0.0.0.0:8122]
    W[twg-web statis di host terpisah] -->|HTTP :8122| S
    S --> P[(PostgreSQL 127.0.0.1:5432)]
    S --> U[/var/lib/twg/uploads]
    S --> A[/var/lib/twg/asset-store]
    S --> T[/var/lib/twg/tile-cache]
    S -. proxy tile .-> OSM[tile.openstreetmap.org]
    S -. proxy tile .-> ESRI[server.arcgisonline.com]
    S --> G[gltf-transform / Node.js]
```

| Komponen | Peran |
|---|---|
| `twg-server-app` | REST API, login/PASETO, WebSocket `/ws`, static `/uploads` dan `/assets`, proxy tile `/tiles`, semua di port 8122 |
| PostgreSQL | Semua data skenario; migrasi SQLx otomatis saat server start |
| `gltf-transform` | Konversi Draco/LOD2 dari UI admin (opsional, tapi dibutuhkan untuk asset pipeline) |
| `twg-desktop` | Client utama; pilih **Publik** atau isi IP VPS lewat **Pengaturan** (lihat bagian 9) |
| `twg-web` | Statis WASM; harus di-host terpisah dan memanggil API port 8122 |

## Yang perlu disiapkan

- VPS Ubuntu 22.04/24.04, **minimal 2 vCPU / 4 GB RAM / 40 GB disk**.
  Build release Rust butuh ~2–4 GB RAM; kalau RAM 2 GB, tambahkan swap 2 GB.
- **Port 22 dan 8122** terbuka di UFW **dan** firewall penyedia VPS
  (Security Group / Firewall Rules).
- Akses `sudo`.
- (Opsional) akses keluar HTTPS ke `tile.openstreetmap.org` dan
  `server.arcgisonline.com` untuk proxy tile.
- (Opsional) IP publik statis atau domain agar client mudah diarahkan.

## Ringkasan langkah

1. Install paket sistem, Rust, Node.js, dan PostgreSQL.
2. Buat user `twg`, direktori runtime, dan database.
3. Clone repo, build `twg-server-app` mode release.
4. Isi `/etc/twg/server.env` dengan bind `0.0.0.0:8122`.
5. Pasang systemd service, start, lalu buat admin pertama lewat SQL.
6. Buka port 8122 di firewall, arahkan client ke `http://IP:8122`.
7. Verifikasi health check dan login.
8. Atur backup, log, dan prosedur upgrade.

---

## 1. Prasyarat sistem

```bash
sudo apt update && sudo apt upgrade -y
sudo apt install -y build-essential pkg-config curl git unzip rsync \
  postgresql postgresql-contrib apache2-utils
```

Firewall (SSH + port aplikasi):

```bash
sudo ufw allow OpenSSH
sudo ufw allow 8122/tcp
sudo ufw enable
sudo ufw status
```

> Jangan lupa membuka **8122/tcp** juga di firewall penyedia VPS. Tanpa itu,
> `ufw` saja tidak cukup dan koneksi dari luar tetap gagal.
>
> Kalau client berasal dari jaringan tetap, batasi sumbernya:
> `sudo ufw allow from 10.20.0.0/16 to any port 8122 proto tcp`.

Rust (pakai user `twg`, jangan root):

```bash
sudo useradd --system --create-home --shell /bin/bash twg
sudo -u twg bash -lc 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal'
sudo -u twg bash -lc 'source $HOME/.cargo/env && rustc --version && cargo --version'
```

Tambahkan swap bila RAM ≤ 2 GB:

```bash
sudo fallocate -l 2G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```

Node.js + `gltf-transform` (hanya perlu bila fitur konversi asset dipakai).
Pasang Node dari NodeSource agar binary global ada di `/usr/bin`:

```bash
curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash -
sudo apt install -y nodejs
sudo npm install -g @gltf-transform/cli
which gltf-transform     # biasanya /usr/bin/gltf-transform
```

> Build `twg-server-app` **tidak** membutuhkan CMake/draco decoder. CMake hanya
> diperlukan untuk build `twg-desktop`/`twg-gltf-draco`.

## 2. Siapkan PostgreSQL

```bash
sudo -u postgres psql <<'SQL'
CREATE ROLE twg LOGIN PASSWORD 'GANTI_PASSWORD_DB';
CREATE DATABASE twg OWNER twg;
SQL
```

Verifikasi:

```bash
psql "postgresql://twg:GANTI_PASSWORD_DB@127.0.0.1:5432/twg?sslmode=disable" -c 'SELECT 1;'
```

Catatan:

- Ekstensi `uuid-ossp` dibuat oleh migrasi `000000_extensions.up.sql`. Di
  PostgreSQL 13+ ekstensi ini *trusted*, jadi owner database boleh membuatnya.
- `sqlx` di workspace dikompilasi **tanpa fitur TLS**. Pakai
  `sslmode=disable` untuk koneksi loopback/privat. Jangan mengarahkan
  `DB_SOURCE` ke database publik dengan `sslmode=require`.
- PostgreSQL hanya mendengarkan `127.0.0.1` (default Ubuntu). Jangan buka 5432.
- Migrasi dijalankan otomatis saat server start. Database kosong akan dibuatkan
  seluruh skema. Database lama yang sudah punya tabel `users` tetapi belum punya
  `_sqlx_migrations` akan **melewati migrasi** (lihat log peringatan) dan hanya
  menambahkan kolom kompatibilitas.

## 3. Direktori, source, dan build

```bash
sudo mkdir -p /opt/twg /etc/twg /var/lib/twg/uploads /var/lib/twg/asset-store /var/lib/twg/tile-cache
sudo chown -R twg:twg /opt/twg /var/lib/twg
sudo chmod 750 /etc/twg
sudo -u twg git clone https://github.com/vinrul/twg-rust.git /opt/twg/src
```

Build release (bisa 5–15 menit tergantung CPU):

```bash
sudo -u twg bash -lc 'cd /opt/twg/src && cargo build --release -p twg-server-app'
sudo cp /opt/twg/src/target/release/twg-server-app /opt/twg/twg-server-app
sudo chown twg:twg /opt/twg/twg-server-app
```

Cek binary:

```bash
file /opt/twg/twg-server-app
ldd /opt/twg/twg-server-app | head
```

Binary menerima satu argumen maintenance, `--repair-assets`. Tanpa argumen ia
menjalankan server.

> Alternatif: build di mesin lain/CI lalu unggah dengan
> `scp target/release/twg-server-app user@vps:/opt/twg/`. Pastikan glibc target
> build ≤ glibc VPS.
>
> **Jangan menyalin binary hasil build Windows (`.exe`) ke VPS.** systemd akan
> gagal dengan `status=203/EXEC`. Cek dengan `file /opt/twg/twg-server-app`:
> hasil yang benar adalah `ELF 64-bit ... x86-64` (atau `aarch64` sesuai VPS),
> bukan `PE32+ executable ... MS Windows`.

## 4. Konfigurasi environment (bind ke 0.0.0.0)

Server membaca environment dari proses, lalu `.env` di working directory, lalu
`app.env` (menimpa `.env`), lalu file `APP_ENV_FILE` bila di-set. Dengan
systemd, pakai `EnvironmentFile`.

Buat `/etc/twg/server.env`:

```ini
# --- Wajib -------------------------------------------------------------
DB_SOURCE=postgresql://twg:GANTI_PASSWORD_DB@127.0.0.1:5432/twg?sslmode=disable
# Tepat 32 byte. WAJIB diganti. Generate: openssl rand -hex 16
# (contoh di bawah hanya placeholder 32 karakter)
TOKEN_SYMMETRIC_KEY=0123456789abcdef0123456789abcdef

# --- Jaringan ----------------------------------------------------------
# WAJIB 0.0.0.0 agar bisa diakses dari luar VPS.
# Default aplikasi 127.0.0.1:8122 dan hanya bisa diakses lokal.
HTTP_SERVER_ADDRESS=0.0.0.0:8122
# Origin frontend web yang diizinkan (comma-separated). Untuk desktop tidak
# berpengaruh. Contoh: http://192.168.1.50:8080 atau https://twg.example.com.
# Gunakan * hanya bila tidak butuh cookie/credentials.
CORS_DOMAIN=*
# Batas body request (upload asset), 100 MiB.
MAX_BODY_BYTES=104857600

# --- Token & sesi ------------------------------------------------------
ACCESS_TOKEN_DURATION=24h
REFRESH_TOKEN_DURATION=168h
SINGLE_DEVICE_ROLES=admin,admin_satker

# --- Rate limit login --------------------------------------------------
# Tanpa reverse proxy, limit dihitung per IP client asli.
AUTH_RATE_LIMIT=10
AUTH_RATE_WINDOW_SECONDS=60

# --- Realtime WebSocket ------------------------------------------------
WS_TICKET_TTL_SECONDS=30
WS_HISTORY_LIMIT=256

# --- Penyimpanan -------------------------------------------------------
UPLOADS_DIR=/var/lib/twg/uploads
ASSETS_DIR=/var/lib/twg/asset-store
TILE_CACHE_DIR=/var/lib/twg/tile-cache

# --- Proxy tile (opsional) --------------------------------------------
TILE_PROXY_CONCURRENCY=8
TILE_PROXY_MIN_INTERVAL_MS=50

# --- Konversi glTF -----------------------------------------------------
GLTF_TRANSFORM_BIN=/usr/bin/gltf-transform
ASSET_LOD2_RATIO=0.5
ASSET_LOD2_ERROR=0.001

# --- Log ---------------------------------------------------------------
RUST_LOG=info,tower_http=info
```

Izin file (berisi secret):

```bash
sudo chown root:twg /etc/twg/server.env
sudo chmod 640 /etc/twg/server.env
```

### Referensi variabel

| Variabel | Wajib | Default | Keterangan |
|---|---|---|---|
| `DB_SOURCE` / `DATABASE_URL` | ya | — | URL PostgreSQL. `sslmode=disable` untuk loopback. |
| `TOKEN_SYMMETRIC_KEY` | ya | — | Tepat 32 byte. Dipakai enkripsi token PASETO. Ganti = semua sesi logout. |
| `HTTP_SERVER_ADDRESS` | ya* | `127.0.0.1:8122` | **Set `0.0.0.0:8122`** untuk akses luar. `[::]:8122` untuk IPv6. |
| `CORS_DOMAIN` | tidak | `http://localhost:5173` | Daftar origin web dipisah koma, atau `*` (tanpa credentials). |
| `ACCESS_TOKEN_DURATION` / `REFRESH_TOKEN_DURATION` | tidak | `24h` / `168h` | Format `s/m/h/d`. |
| `SINGLE_DEVICE_ROLES` | tidak | `admin,admin_satker` | Role yang hanya boleh punya satu sesi aktif. |
| `MAX_BODY_BYTES` | tidak | `104857600` | Batas body upload. |
| `UPLOADS_DIR` | tidak | `uploads` | File upload gambar/video. Disajikan di `/uploads`. |
| `ASSETS_DIR` | tidak | `asset-store` | File asset library. Disajikan di `/assets`. |
| `TILE_CACHE_DIR` | tidak | `.tile-cache` | Cache tile hasil proxy. |
| `TILE_PROXY_CONCURRENCY` | tidak | `8` | Jumlah fetch tile paralel. |
| `TILE_PROXY_MIN_INTERVAL_MS` | tidak | `50` | Jeda minimum antar-fetch ke provider tile. |
| `GLTF_TRANSFORM_BIN` | tidak | `gltf-transform` | Path/nama binary `gltf-transform`. |
| `ASSET_LOD2_RATIO` / `ASSET_LOD2_ERROR` | tidak | `0.5` / `0.001` | Parameter simplifikasi LOD2. |
| `AUTH_RATE_LIMIT` / `AUTH_RATE_WINDOW_SECONDS` | tidak | `10` / `60` | Rate limit login per IP client. |
| `WS_TICKET_TTL_SECONDS` | tidak | `30` | Umur tiket WebSocket (5–300 detik). |
| `WS_HISTORY_LIMIT` | tidak | `256` | Jumlah pesan realtime yang disimpan per kanal. |
| `RUST_LOG` | tidak | `info,tower_http=info` | Filter tracing. |
| `APP_ENV_FILE` | tidak | — | File env tambahan yang menimpa yang lain. |

\* Wajib di-set eksplisit untuk deploy ini; default hanya listen loopback.

> `ENVIROMENT` (salah eja, warisan Go), `CENTRIFUGO_*`, dan `CHANNEL` di
> `.env.example` **tidak** dibaca oleh server Rust. Jangan diisi di produksi
> kecuali kamu memang menjalankan komponen legacy.

## 5. systemd service

`/etc/systemd/system/twg-server.service`:

```ini
[Unit]
Description=TWG Server (twg-server-app)
After=network-online.target postgresql.service
Wants=network-online.target

[Service]
Type=simple
User=twg
Group=twg
WorkingDirectory=/opt/twg
EnvironmentFile=/etc/twg/server.env
ExecStart=/opt/twg/twg-server-app
Restart=on-failure
RestartSec=2
TimeoutStopSec=30
LimitNOFILE=65536
UMask=0077

# Hardening. Jangan tambahkan MemoryDenyWriteExecute: proses anak Node
# (gltf-transform) membutuhkannya untuk JIT.
NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectSystem=strict
ProtectHome=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictSUIDSGID=true
LockPersonality=true
# AF_NETLINK dibutuhkan Node (gltf-transform) untuk enumerasi interface.
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX AF_NETLINK
ReadWritePaths=/var/lib/twg

[Install]
WantedBy=multi-user.target
```

Aktifkan:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now twg-server
sudo systemctl status twg-server --no-pager
sudo journalctl -u twg-server -f
```

Log awal yang sehat kira-kira:

```text
loaded server configuration address=0.0.0.0:8122 cors=[...] max_body_bytes=104857600
starting twg server address=0.0.0.0:8122
```

Pastikan listening di semua interface (bukan `127.0.0.1`):

```bash
ss -ltnp | grep 8122
# LISTEN 0 1024 0.0.0.0:8122 ... users:(("twg-server-app",...))
```

Cek dari dalam VPS:

```bash
curl -fsS http://127.0.0.1:8122/healthz     # {"status":"ok"}
curl -fsS http://203.0.113.10:8122/healthz  # dari luar juga harus 200
curl -fsS http://127.0.0.1:8122/readyz      # 503 bila database belum siap
```

`/healthz` selalu 200 bila proses hidup. `/readyz` melakukan ping PostgreSQL
dan mengembalikan **503** bila database tidak bisa dihubungi.

## 6. Admin pertama

Server tidak punya endpoint bootstrap publik. Setelah migrasi berjalan, buat
admin langsung di database dengan hash bcrypt (cost 10, kompatibel dengan
verifikasi server):

```bash
HASH=$(htpasswd -bnBC 10 "" 'GANTI_PASSWORD_ADMIN' | tr -d ':\n' | sed 's/^\$2y\$/\$2a\$/')
sudo -u postgres psql -d twg <<SQL
INSERT INTO users (name, username, password, role)
VALUES ('Administrator', 'admin', '${HASH}', 'admin')
ON CONFLICT (username) DO UPDATE
SET password = EXCLUDED.password, role = 'admin';
SQL
```

Cek login lewat API (response dibungkus `{code, message, data}`):

```bash
curl -fsS -X POST http://203.0.113.10:8122/auth/login \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"GANTI_PASSWORD_ADMIN"}'
```

Endpoint `/private/*` membutuhkan header `Authorization: Bearer <access_token>`
**dan** `x-device-id: <device_id>` yang sama seperti saat login (boleh dikirim
di body login atau lewat header).

Setelah login, kategori, satker, operasi, skenario, dan asset dibuat lewat UI
admin/desktop — tidak perlu seeding manual.

## 7. Firewall, akses, dan batasan tanpa TLS

Tanpa reverse proxy, port 8122 adalah satu-satunya pintu masuk:

```bash
sudo ufw status
sudo ufw allow 8122/tcp
# batasi bila client dari jaringan tetap:
# sudo ufw allow from 10.20.0.0/16 to any port 8122 proto tcp
```

Uji dari komputer lain:

```bash
curl -v http://203.0.113.10:8122/healthz
# curl: (7) Failed to connect  → cek HTTP_SERVER_ADDRESS=0.0.0.0:8122,
#                                ufw, dan firewall penyedia VPS
```

Batasan yang harus disadari:

- **Tidak ada TLS.** Password dan token terkirim sebagai HTTP biasa. Jangan
  pakai di jaringan publik tanpa tambahan pengaman.
- **Web dari HTTPS tidak bisa memanggil API HTTP.** Browser memblokir sebagai
  *mixed content*. Web statis harus di-host lewat HTTP (tidak disarankan) atau
  API juga harus HTTPS (lihat [Lampiran A](#lampiran-a-nginx--tls-opsional)).
- **Desktop tidak terpengaruh mixed content**; `http://IP:8122` langsung
  berfungsi. WebSocket otomatis memakai `ws://` karena URL dibentuk dari base
  API (`replace("http", "ws")`).
- **Rate limit per IP asli** sekarang berfungsi normal karena tidak ada proxy.
- **Tidak ada kompresi, HTTP/2, atau cache aset** di depan server. Untuk
  kebutuhan itu, pasang reverse proxy.
- **Port 8122 terbuka ke internet** = siapa pun bisa mencoba login (dibatasi
  rate limit). Persempit dengan firewall/VPN.

## 8. Web statis (opsional)

`twg-server-app` **tidak** melayani `twg-web`. Build WASM dan host terpisah:

```bash
sudo -u twg bash -lc '
  source $HOME/.cargo/env &&
  rustup target add wasm32-unknown-unknown &&
  cargo install trunk --locked &&
  cd /opt/twg/src/apps/web &&
  trunk build --release
'
```

Hasilnya di `/opt/twg/src/apps/web/dist/`. Pilihan hosting:

- **Server statis terpisah** (misalnya Caddy/Nginx di VPS lain, atau hosting
  statis internal) yang menyajikan folder `dist`.
- **Pengujian lokal**: `python3 -m http.server 8080 --directory dist` (jangan
  dipakai produksi).

Arahkan web ke API port 8122:

```text
http://192.168.1.50:8080/?api=http://203.0.113.10:8122
```

atau ubah default di `apps/web` (`api_base_url()` di
`crates/twg-client/src/web.rs`), lalu build ulang.

Catatan:

- `CORS_DOMAIN` harus memuat origin web (`http://192.168.1.50:8080`). Kalau web
  dan API beda host, CORS wajib benar; kalau `CORS_DOMAIN=*`, credentials
  dimatikan (aplikasi memakai token di header, jadi masih berfungsi).
- Service worker `tile-cache-sw.js` membutuhkan secure context (HTTPS atau
  localhost). Di HTTP biasa, service worker tidak terdaftar; tile tetap
  dimuat, hanya tanpa cache browser.
- Memuat web dari halaman HTTPS ke API HTTP akan gagal (*mixed content*).
  Solusinya TLS di depan API (Lampiran A) atau host web di HTTP pada jaringan
  internal.

## 9. Client desktop

Di komputer operator, jalankan aplikasi tanpa konfigurasi tambahan:

```bash
cargo run -p twg-desktop --release
```

Saat pertama dibuka, desktop memeriksa koneksi sesuai pilihan tersimpan, lalu
menampilkan halaman login. Pilihan koneksi ada di halaman login dan menu
**Pengaturan**:

- **Publik** — server tetap `103.179.218.76:8122` (alamat tidak ditampilkan di
  UI).
- **Lokal** — isi IP/host dan port server sendiri, mis. `127.0.0.1:8122` atau
  `203.0.113.10:8122`.
- **Simpan** menyimpan pilihan ke berkas per komputer
  (`%APPDATA%\twg\connection.json` di Windows,
  `~/.config/twg/connection.json` di Linux) lalu memeriksa ulang server.
  Menyimpan koneksi baru mengakhiri sesi lama.
- **Tes** menguji alamat pada form tanpa menyimpannya.

Bila server tidak terjangkau, halaman login menampilkan pesan koneksi gagal;
pindah pilihan koneksi lalu klik **Simpan**.

`TWG_API_URL` masih bisa dipakai sebagai override (untuk skrip/CI) dan menang
atas pengaturan tersimpan. `TWG_TILE_PROXY` opsional; bila kosong, desktop
memakai server API sebagai proxy tile sehingga mengikuti pilihan koneksi.

## 10. Verifikasi (smoke test)

```bash
API=http://203.0.113.10:8122

# 1. Proses & port
sudo systemctl is-active twg-server
ss -ltnp | grep 8122            # harus 0.0.0.0:8122

# 2. Health
curl -fsS "$API/healthz"
curl -fsS "$API/readyz"

# 3. Login
TOKEN=$(curl -fsS -X POST "$API/auth/login" \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"GANTI_PASSWORD_ADMIN","device_id":"ops-1"}' \
  | sed -n 's/.*"access_token":"\([^"]*\)".*/\1/p')
echo "${TOKEN:0:16}..."

# 4. Endpoint privat (wajib Bearer + x-device-id)
curl -fsS "$API/private/operasis" \
  -H "authorization: Bearer $TOKEN" \
  -H 'x-device-id: ops-1'

# 5. Static uploads/assets (server sendiri)
curl -I "$API/assets/models/polwan_airud_idle_lod2.glb"

# 6. Proxy tile (ambil satu tile OSM)
curl -fsS -o /tmp/tile.png "$API/tiles/osm/15/26400/17100.png"
file /tmp/tile.png

# 7. WebSocket: tanpa tiket akan 400/401 — yang penting koneksi TCP/HTTP
#    sampai ke server, bukan connection refused.
curl -i "$API/ws" | head
```

Uji upload besar lewat UI admin/desktop, bukan `curl`, karena endpoint asset
memakai multipart.

## 11. Operasional

### 11.1 Upgrade

```bash
sudo -u twg git -C /opt/twg/src pull --ff-only
sudo -u twg bash -lc 'cd /opt/twg/src && cargo build --release -p twg-server-app'
sudo cp /opt/twg/twg-server-app /opt/twg/twg-server-app.prev 2>/dev/null || true
sudo install -o twg -g twg -m 755 /opt/twg/src/target/release/twg-server-app /opt/twg/twg-server-app
sudo systemctl restart twg-server
sudo journalctl -u twg-server -n 100 --no-pager
```

- Migrasi SQLx berjalan otomatis saat start. Tetap **backup dulu**.
- Setelah upgrade yang menyentuh format asset, jalankan sekali:

```bash
sudo -u twg /opt/twg/twg-server-app --repair-assets
```

Perintah ini memperbaiki mimeType gambar di dalam GLB, memperbarui
`size_bytes`/`sha256`, menaikkan `version` asset, lalu keluar.

- Rollback binary:

```bash
sudo cp /opt/twg/twg-server-app.prev /opt/twg/twg-server-app
sudo systemctl restart twg-server
```

### 11.2 Backup

```bash
sudo mkdir -p /var/backups/twg
sudo pg_dump -Fc -U twg twg > /var/backups/twg/twg-$(date +%F).dump
sudo tar -C /var/lib/twg -czf /var/backups/twg/files-$(date +%F).tar.gz uploads asset-store
```

Cron contoh (02:30, simpan 14 hari):

```cron
30 2 * * * root pg_dump -Fc -U twg twg > /var/backups/twg/twg-$(date +\%F).dump && find /var/backups/twg -name 'twg-*.dump' -mtime +14 -delete
45 2 * * * root tar -C /var/lib/twg -czf /var/backups/twg/files-$(date +\%F).tar.gz uploads asset-store && find /var/backups/twg -name 'files-*.tar.gz' -mtime +14 -delete
```

`tile-cache` tidak perlu di-backup. Simpan backup di luar VPS.

### 11.3 Log dan disk

```bash
sudo journalctl -u twg-server --since '1 hour ago' --no-pager
sudo du -sh /var/lib/twg/*
sudo find /var/lib/twg/tile-cache -type f -mtime +30 -delete
```

`RUST_LOG=debug` untuk investigasi sementara; kembalikan ke `info` setelahnya.

## 12. Keamanan & batasan penting

- **Tidak ada TLS di server.** Gunakan firewall/VPN, atau TLS di depan (Lampiran
  A). Jangan kirim password lewat internet terbuka.
- **Satu instance.** Rate limiter login, tiket WebSocket, riwayat realtime, dan
  idempotency disimpan di memori proses. Untuk dua instance ke atas, gunakan
  load balancer dengan sticky session `/ws` atau pindahkan state ke Redis
  (belum diimplementasi).
- **Token dan password** hanya dilindungi HTTPS. `TOKEN_SYMMETRIC_KEY` tepat 32
  byte dan jangan dibagikan; menggantinya membatalkan semua sesi.
- `CORS_DOMAIN=*` mematikan credentials. Aplikasi memakai token di header, jadi
  tetap berfungsi, tetapi origin terbuka. Lebih baik sebutkan origin web secara
  eksplisit bila web dipakai.
- **Port 8122** sebaiknya dibatasi ke IP client (`ufw allow from ...`). Server
  tidak punya WAF/rate limit selain login.
- `/uploads` dan `/assets` disajikan dengan
  `Content-Security-Policy: default-src 'none'; sandbox` dan
  `Cross-Origin-Resource-Policy: same-site`.
- Proxy tile meneruskan permintaan ke OSM/Esri dengan User-Agent tetap dan
  throttle `TILE_PROXY_MIN_INTERVAL_MS`. Untuk trafik besar gunakan provider
  komersial/tile server sendiri; patuhi kebijakan OSM.
- Password minimal 8 byte, maksimal 72 byte (batas bcrypt), cost 10.
- Sesi `admin`/`admin_satker` single-device; login kedua mencabut sesi pertama.
  `twg-online-admin` dapat memantau dan melakukan kick.

## 13. Troubleshooting (akses langsung)

| Gejala | Penyebab & solusi |
|---|---|
| `curl: (7) Failed to connect` dari luar | `HTTP_SERVER_ADDRESS` masih `127.0.0.1`, port 8122 belum dibuka di `ufw`, atau firewall penyedia VPS belum dibuka. Cek `ss -ltnp \| grep 8122`. |
| Service gagal start, log `missing environment variable` | `DB_SOURCE`/`TOKEN_SYMMETRIC_KEY` tidak terbaca. Cek `EnvironmentFile` dan izin `/etc/twg/server.env`. |
| `TOKEN_SYMMETRIC_KEY must be exactly 32 bytes` | Hasilkan dengan `openssl rand -hex 16` (32 karakter). |
| `/readyz` 503 | PostgreSQL mati/salah password. Uji `psql` dengan URL yang sama. |
| Login 429 | Rate limit per IP tercapai. Naikkan `AUTH_RATE_LIMIT` atau tunggu window. |
| Request privat 401 | Header `Authorization: Bearer` atau `x-device-id` tidak dikirim / device tidak cocok dengan sesi. |
| Upload 413 | Naikkan `MAX_BODY_BYTES`, lalu restart service. |
| Web dari HTTPS error `Mixed Content` / API diblokir | Halaman HTTPS tidak boleh memanggil API HTTP. Pasang TLS di depan API (Lampiran A) atau host web via HTTP internal. |
| CORS error di browser | `CORS_DOMAIN` harus memuat origin web persis (skema + host + port, tanpa slash). Restart service setelah diubah. |
| WebSocket gagal dari web | Pastikan halaman HTTP (bukan HTTPS) dan `CORS_DOMAIN` benar. URL WS dibentuk dari base API menjadi `ws://IP:8122/ws`. |
| WebSocket gagal dari desktop | `TWG_API_URL` harus `http://IP:8122` (bukan `https`) selama server tanpa TLS. |
| Tile 503/timeout | VPS tidak bisa akses internet, atau provider membatasi. Cek `journalctl`, kurangi `TILE_PROXY_CONCURRENCY`. |
| Tile 404 setelah ganti area | Normal: tile belum ter-cache dan provider belum punya zoom tersebut. |
| Konversi asset gagal, log menyebut `gltf-transform` | Node/CLI tidak terpasang atau `GLTF_TRANSFORM_BIN` salah. Uji `sudo -u twg gltf-transform --version`. |
| Log `existing legacy schema detected ... skipping migrations` | Database berasal dari server lama. Untuk produksi baru pakai database kosong. |
| Binary error glibc saat start | Binary di-build di distro lebih baru. Build ulang di VPS atau Ubuntu versi sama. |
| Service gagal dengan `status=203/EXEC` | systemd tidak bisa mengeksekusi file (aplikasi belum jalan). Lihat [subsection 13.1](#131-status203exec). |

### 13.1 `status=203/EXEC`

`203/EXEC` berarti `execve()` gagal: path salah, file tidak executable, format
binary salah (Windows/arsitektur lain), loader tidak ada, atau file tersembunyi
oleh hardening. Aplikasi tidak sempat menulis log apa pun.

```bash
systemctl cat twg-server | grep ExecStart
ls -l /opt/twg/twg-server-app
file /opt/twg/twg-server-app
uname -m
sudo -u twg test -x /opt/twg/twg-server-app && echo executable || echo NOT-executable
sudo -u twg /opt/twg/twg-server-app 2>&1 | head -20   # Ctrl+C bila benar-benar start
ldd /opt/twg/twg-server-app | head
ldd --version | head -1
findmnt -T /opt/twg/twg-server-app -o TARGET,OPTIONS
```

| Temuan | Penyebab | Perbaikan |
|---|---|---|
| `file` → `PE32+ ... MS Windows` | Binary build Windows terunggah | Build ulang di VPS |
| `file` → arsitektur beda dari `uname -m` | Arch tidak cocok | Build ulang / cross-compile |
| File tidak ada / symlink rusak | Path `ExecStart` salah | `sudo install -o twg -g twg -m 755 target/release/twg-server-app /opt/twg/twg-server-app` |
| `NOT-executable` | Izin hilang setelah `scp`/`cp` | `sudo chmod 755` + `sudo chown twg:twg` |
| `No such file or directory` padahal file ada | glibc/loader tidak cocok (build di distro lebih baru atau VPS musl) | Build di VPS itu sendiri, atau musl static |
| `GLIBC_2.xx not found` | glibc VPS lebih lama | Build di VPS/distro sama |
| `findmnt` menyebut `noexec` | Mount filesystem melarang eksekusi | Pindahkan binary ke `/opt/twg` |
| `ExecStart` di `/home/...` | Unit memakai `ProtectHome=true` | Pindahkan binary ke `/opt/twg` atau set `ProtectHome=false` |
| `audit: denied` di `dmesg` | SELinux/AppArmor | Perbaiki label/context (`restorecon -Rv /opt/twg`) |

Perbaikan cepat:

```bash
sudo systemctl stop twg-server
sudo install -o twg -g twg -m 755 \
  /opt/twg/src/target/release/twg-server-app /opt/twg/twg-server-app
sudo -u twg /opt/twg/twg-server-app        # uji manual, lalu Ctrl+C
sudo systemd-analyze verify /etc/systemd/system/twg-server.service
sudo systemctl reset-failed twg-server
sudo systemctl restart twg-server
sudo journalctl -u twg-server -n 50 --no-pager
```

## Lampiran A: Nginx + TLS (opsional)

Pasang ini nanti bila butuh HTTPS, web satu domain, kompresi, atau cache tile.
Pada mode ini server cukup listen loopback.

1. Install Nginx + certbot:

```bash
sudo apt install -y nginx certbot python3-certbot-nginx
```

2. Ubah `/etc/twg/server.env`:

```ini
HTTP_SERVER_ADDRESS=127.0.0.1:8122
CORS_DOMAIN=https://twg.example.com
```

3. `/etc/nginx/snippets/twg-proxy.conf`:

```nginx
proxy_http_version 1.1;
proxy_set_header Host $host;
proxy_set_header X-Real-IP $remote_addr;
proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
proxy_set_header X-Forwarded-Proto $scheme;
proxy_connect_timeout 10s;
proxy_send_timeout 600s;
proxy_read_timeout 600s;
proxy_request_buffering off;
```

> Server membaca IP klien dari socket, bukan `X-Forwarded-For`. Di balik Nginx
> semua request terlihat `127.0.0.1`; naikkan `AUTH_RATE_LIMIT` (mis. `100`).

4. `/etc/nginx/sites-available/twg` (ringkas):

```nginx
map $http_upgrade $connection_upgrade {
    default upgrade;
    ''      close;
}

upstream twg_api {
    server 127.0.0.1:8122;
    keepalive 32;
}

server {
    listen 80;
    listen [::]:80;
    server_name twg.example.com;

    root /var/www/twg;
    index index.html;
    client_max_body_size 128m;
    client_body_timeout 300s;

    # Web statis /assets/models/... vs asset library /assets/<uuid>/...
    location /assets/ {
        try_files $uri @api;
    }

    location /uploads/ {
        proxy_pass http://twg_api;
        include /etc/nginx/snippets/twg-proxy.conf;
    }

    location /tiles/ {
        proxy_pass http://twg_api;
        include /etc/nginx/snippets/twg-proxy.conf;
        proxy_cache twg_tiles;
        proxy_cache_valid 200 7d;
        proxy_cache_valid 404 1h;
        add_header X-Cache-Status $upstream_cache_status;
    }

    location /ws {
        proxy_pass http://twg_api;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $connection_upgrade;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_read_timeout 3600s;
        proxy_send_timeout 3600s;
        proxy_buffering off;
    }

    location ~ ^/(auth|private|public|admin)/ {
        proxy_pass http://twg_api;
        include /etc/nginx/snippets/twg-proxy.conf;
    }

    location = /healthz { proxy_pass http://twg_api; }
    location = /readyz  { proxy_pass http://twg_api; }

    location @api {
        proxy_pass http://twg_api;
        include /etc/nginx/snippets/twg-proxy.conf;
    }

    location / {
        try_files $uri $uri/ /index.html;
    }
}
```

Cache tile (opsional), `/etc/nginx/conf.d/twg-tile-cache.conf`:

```nginx
proxy_cache_path /var/cache/nginx/twg_tiles
    levels=1:2 keys_zone=twg_tiles:64m max_size=10g inactive=30d use_temp_path=off;
```

5. Aktifkan, TLS, dan web statis:

```bash
sudo ln -s /etc/nginx/sites-available/twg /etc/nginx/sites-enabled/twg
sudo rm -f /etc/nginx/sites-enabled/default
sudo nginx -t && sudo systemctl reload nginx
sudo certbot --nginx -d twg.example.com --redirect

# build web (opsional)
sudo -u twg bash -lc 'cd /opt/twg/src/apps/web && trunk build --release'
sudo mkdir -p /var/www/twg
sudo rsync -a --delete /opt/twg/src/apps/web/dist/ /var/www/twg/
sudo chown -R www-data:www-data /var/www/twg
sudo systemctl restart twg-server
```

6. Tutup akses langsung 8122:

```bash
sudo ufw delete allow 8122/tcp
```

Setelah TLS aktif, client memakai `https://twg.example.com` dan tidak perlu
`?api=`. Alternatif yang lebih sederhana: **Caddy** dengan konfigurasi
`reverse_proxy 127.0.0.1:8122` + `file_server`, atau **Cloudflare Tunnel** bila
tidak ingin membuka port sama sekali.

## Checklist akhir (akses langsung)

- [ ] `HTTP_SERVER_ADDRESS=0.0.0.0:8122` dan `ss -ltnp` menunjukkan
      `0.0.0.0:8122`.
- [ ] Port 8122 dibuka di `ufw` **dan** firewall penyedia VPS (atau dibatasi ke
      IP client).
- [ ] `curl http://IP:8122/healthz` dan `/readyz` = 200 dari luar VPS.
- [ ] `sudo systemctl is-enabled twg-server` = enabled.
- [ ] Login `admin` berhasil dan `/private/operasis` mengembalikan data.
- [ ] Upload asset besar berhasil lewat UI admin/desktop.
- [ ] WebSocket `/ws` terhubung dan realtime berjalan di dua client desktop.
- [ ] Proxy tile mengembalikan PNG dan `tile-cache` terisi.
- [ ] Backup harian berjalan dan file hasilnya bisa direstore.
- [ ] `RUST_LOG` kembali ke `info` setelah selesai investigasi.
- [ ] Bila web dipakai dari HTTPS: TLS di depan API sudah aktif (Lampiran A).
