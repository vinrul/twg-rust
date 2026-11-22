# Release aplikasi desktop

Dua aplikasi GUI desktop:

| Aplikasi | Binary | Ikon |
|---|---|---|
| Tactical Game 3D | `twg-desktop` | `assets/icons/twg-desktop.*` |
| TWG Online Admin | `twg-online-admin` | `assets/icons/twg-admin.*` |

Build **release** tidak membuka jendela console di Windows. Build **debug**
tetap menampilkan console agar log Bevy mudah dibaca.

Saat aplikasi berjalan, ikon window dipasang dari `twg-*-ui.rgba`: winit
memakai `set_window_icon` (title bar Windows, X11) dan `set_taskbar_icon`
(ICON_BIG Windows), sehingga ikon muncul di title bar dan taskbar, bukan hanya
di file `.exe`. macOS memakai ikon bundle, Wayland memakai ikon `.desktop`.

## Ikon

Ikon digambar oleh `tools/gen-icons.mjs` (Node.js, tanpa dependensi) dan
disimpan sebagai:

- `assets/icons/twg-*-16.png` … `twg-*-1024.png` — Linux/hicolor dan macOS.
- `assets/icons/twg-desktop.ico`, `twg-admin.ico` — resource Windows
  (16–256 px).
- `assets/icons/twg-desktop.icns`, `twg-admin.icns` — macOS (32–1024 px).
- `assets/icons/twg-*-ui.rgba` — 256×256 RGBA mentah untuk ikon window saat
  runtime (`winit::window::Icon`), disematkan lewat `include_bytes!`.

Regenerasi setelah mengubah artwork:

```bash
node tools/gen-icons.mjs
```

Palet: desktop memakai tile hijau terang dengan pin peta + crosshair, admin
memakai tile orange terang dengan radar + blip user; glyph navy gelap agar
tetap kontras di tema terang maupun gelap.

## Windows

Ikon disematkan ke `.exe` oleh `apps/desktop/build.rs` dan
`apps/online-admin/build.rs` menggunakan crate `winresource`; file `.ico` tidak
perlu ikut didistribusikan. `windows_subsystem = "windows"` pada build release
membuat aplikasi tidak membuka terminal.

```powershell
powershell -ExecutionPolicy Bypass -File tools/package-windows.ps1
```

Hasil: `dist/windows/` berisi kedua `.exe` + ikon untuk shortcut, dan
`dist/twg-windows-x64.zip`.

Catatan:

- Build harus dijalankan di Windows (atau cross-compile dengan resource
  compiler Windows yang tersedia). `build.rs` hanya menyematkan ikon saat
  target adalah Windows.
- Menjalankan `cargo run --release -p twg-desktop` juga memakai subsystem
  Windows, sehingga log tidak muncul di terminal.
- Code signing belum termasuk; SmartScreen akan menampilkan peringatan sampai
  binary ditandatangani.

## Linux

Build Linux memerlukan header ALSA, udev, dan Wayland/X11 yang dipakai
`twg-desktop` lewat Bevy. Pasang sekali saja (Debian/Ubuntu/WSL):

```bash
sudo apt install -y pkg-config libasound2-dev libudev-dev libwayland-dev \
  libx11-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev
```

Tanpa paket tersebut `cargo build` berhenti di `alsa-sys` dengan
`Package alsa was not found`. Kedua script packaging di bawah memanggil
`tools/check-linux-deps.sh` lebih dulu agar kekurangan paket terdeteksi sebelum
kompilasi panjang dimulai.

Binary Linux tidak menyimpan ikon di dalam executable; ikon dipasang lewat
XDG. Bundle menyertakan `.desktop` dengan `Terminal=false`, sehingga aplikasi
dibuka tanpa terminal dari menu aplikasi.

```bash
bash tools/package-linux.sh
```

Hasil: `dist/linux/` berisi:

```text
bin/twg-desktop
bin/twg-online-admin
share/applications/twg-desktop.desktop
share/applications/twg-online-admin.desktop
share/icons/hicolor/<size>x<size>/apps/twg-desktop.png
share/icons/hicolor/<size>x<size>/apps/twg-admin.png
install.sh
```

Pasang ke `~/.local` (atau `PREFIX=/usr/local ./install.sh`):

```bash
cd dist/linux
./install.sh
```

Arsip siap distribusi: `dist/twg-linux-<arch>.tar.gz`.

### AppImage (untuk auto-update)

Updater Linux mendeteksi env `APPIMAGE` dan mengganti file AppImage saat
update. Build AppImage membutuhkan `appimagetool`. Di Ubuntu/WSL pasang ke
`~/.local/bin` tanpa sudo:

```bash
mkdir -p ~/.local/bin
curl -fL -o ~/.local/bin/appimagetool \
  https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
chmod +x ~/.local/bin/appimagetool
appimagetool --version
```

`appimagetool` sendiri berbentuk AppImage, jadi butuh FUSE 2: Ubuntu 22.04
memakai `sudo apt install libfuse2`, Ubuntu 24.04 memakai `libfuse2t64`. Bila
FUSE tidak tersedia (mis. WSL1), jalankan dengan ekstrak tanpa mount:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 APPIMAGETOOL=$HOME/.local/bin/appimagetool \
  bash tools/package-linux-appimage.sh
```

Tanpa `APPIMAGE_EXTRACT_AND_RUN`, cukup:

```bash
bash tools/package-linux-appimage.sh
```

Hasil: `dist/twg-desktop-<version>-linux-<arch>.AppImage`. File ini yang
dipublikasikan sebagai artefak `linux-<arch>-appimage` di manifest update.

## macOS

`.app` bundle memakai `Info.plist` + `AppIcon.icns`. Membuka bundle dari Finder
tidak membuka Terminal.

```bash
bash tools/package-macos.sh
```

Hasil:

- `dist/macos/Tactical Game 3D.app`
- `dist/macos/TWG Online Admin.app`
- `dist/TacticalGame3D-macos.zip`, `dist/TWGOnlineAdmin-macos.zip` (bila
  `ditto` tersedia)

Bundle belum ditandatangani/notarized. Untuk distribusi publik, jalankan
`codesign` dan `notarytool`, atau pengguna menghapus atribut quarantine:

```bash
xattr -dr com.apple.quarantine "Tactical Game 3D.app"
```

## Auto-update

`twg-desktop` memeriksa manifest bertanda tangan dari server publik sebelum
login. Detail arsitektur ada di [AUTO_UPDATE.md](AUTO_UPDATE.md).

1. Saat build release, set nomor build yang naik:

   ```bash
   TWG_BUILD_NUMBER=87 cargo build --release -p twg-desktop
   ```

   Tanpa env tersebut, build memakai jumlah commit git.

2. Buat manifest + signature (private key dari `dist/keys/`, jangan commit):

   ```bash
   export TWG_UPDATE_SIGNING_KEY=dist/keys/twg-update-signing-key.pem
   node tools/publish-update.mjs \
     --version 0.2.0 --build 87 --notes "..." \
     --windows dist/windows/twg-desktop.exe \
     --linux-appimage dist/twg-desktop-0.2.0-linux-x86_64.AppImage \
     --linux target/release/twg-desktop \
     --macos-arm dist/TacticalGame3D-macos.zip \
     --out dist/updates/twg-desktop \
     --base-url http://103.179.218.76:8122/updates/twg-desktop
   ```

3. Upload ke VPS:

   ```bash
   scp -r dist/updates/twg-desktop/* \
     user@103.179.218.76:/var/lib/twg/updates/twg-desktop/
   ```

4. Uji dengan versi lama: jalankan client lama, pastikan layar update muncul,
   unduhan terverifikasi, aplikasi restart, lalu masuk ke login.

Catatan:

- Public key ada di `assets/update-public-key.txt`. Mengganti key berarti
  client lama tidak bisa memverifikasi update baru dan harus dibangun ulang.
- Build debug hanya memeriksa update jika `TWG_UPDATE_URL` diset; gunakan
  `TWG_SKIP_UPDATE=1` untuk melewati updater saat pengembangan.
- `TWG_UPDATE_CACHE` dapat mengganti folder unduhan sementara.

## Alur rilis multi-platform

`publish-update.mjs` **hanya dijalankan sekali** untuk satu rilis. Manifest
hanya punya satu `stable.json` + satu signature dan harus memuat semua
platform. Kalau dijalankan terpisah di Windows, macOS, dan WSL, setiap run akan
menimpa manifest dengan platform miliknya saja — client di platform lain
kehilangan update.

Urutannya: **build di tiap OS → kumpulkan artefak ke satu mesin → publish
sekali → upload**. Yang memegang private key hanya mesin koordinator.

### 1. Siapkan source dan nomor build

- Commit/tag source, lalu pull commit yang sama di Windows, macOS, dan WSL.
- Tentukan nomor build yang sama untuk semua mesin, mis. `43`. Naikkan setiap
  rilis.
- Jangan jalankan `--keygen` lagi; public key harus sama di semua build.

### 2. Build di tiap OS

Windows (PowerShell):

```powershell
$env:TWG_BUILD_NUMBER = "43"
powershell -ExecutionPolicy Bypass -File tools/package-windows.ps1
```

macOS M1:

```bash
export TWG_BUILD_NUMBER=43
bash tools/package-macos.sh
```

WSL Ubuntu (clone repo di filesystem ext4, mis. `~/twgrev`, bukan `/mnt/...`):

```bash
export TWG_BUILD_NUMBER=43
bash tools/package-linux-appimage.sh
# binary mentah untuk opsi --linux:
# target/release/twg-desktop
```

Build number **harus sama** di ketiga mesin. Kalau Windows build 43 tetapi
macOS build 44, manifest hanya bisa menyebut satu angka dan client di platform
lain akan salah membandingkan versi.

### 3. Kumpulkan artefak di satu mesin

Contoh staging di Windows:

```text
C:\twg-release\
  windows\twg-desktop.exe
  linux\twg-desktop-0.2.0-linux-x86_64.AppImage
  linux\twg-desktop
  macos\TacticalGame3D-macos.zip
```

Copy dari macOS/WSL lewat shared folder, USB, atau `scp`. Dari WSL bisa langsung
ke `/mnt/c/Users/...`. Pastikan nama file tidak berubah.

### 4. Publish sekali (di mesin koordinator)

Di Windows (atau mesin lain yang memegang private key):

```powershell
$env:TWG_UPDATE_SIGNING_KEY = "C:\keys\twg-update-signing-key.pem"

node tools/publish-update.mjs `
  --version 0.2.0 --build 43 --notes "Perbaikan ..." `
  --windows C:\twg-release\windows\twg-desktop.exe `
  --linux-appimage C:\twg-release\linux\twg-desktop-0.2.0-linux-x86_64.AppImage `
  --linux C:\twg-release\linux\twg-desktop `
  --macos-arm C:\twg-release\macos\TacticalGame3D-macos.zip `
  --out dist/updates/twg-desktop `
  --base-url http://103.179.218.76:8122/updates/twg-desktop
```

Satu manifest akan memuat semua platform dan ditandatangani sekali.

### 5. Upload ke VPS

Upload artefak dulu, lalu signature, lalu manifest terakhir supaya client tidak
pernah melihat manifest yang menunjuk file yang belum ada:

```bash
scp -r dist/updates/twg-desktop/0.2.0 \
  user@103.179.218.76:/var/lib/twg/updates/twg-desktop/
scp dist/updates/twg-desktop/stable.json.sig \
  user@103.179.218.76:/var/lib/twg/updates/twg-desktop/
scp dist/updates/twg-desktop/stable.json \
  user@103.179.218.76:/var/lib/twg/updates/twg-desktop/
```

### 6. Verifikasi

Jalankan client versi lama di tiap platform dan pastikan layar update muncul,
unduhan terverifikasi, dan aplikasi restart. Cek juga manifest publik:

```bash
curl -fsS http://103.179.218.76:8122/updates/twg-desktop/stable.json
```

Catatan:

- MacBook M1 menghasilkan `macos-aarch64`. Untuk Intel Mac, build terpisah
  (`--target x86_64-apple-darwin`) lalu publish dengan `--macos-x64`.
- WSL di PC x86_64 menghasilkan `linux-x86_64`. Build di Ubuntu yang lebih tua
  untuk kompatibilitas glibc/AppImage.
- Private key hanya perlu ada di mesin koordinator; mesin build lain tidak
  membutuhkannya.
- macOS belum ditandatangani; Gatekeeper masih bisa memblokir update.

## Daftar periksa rilis

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test -p twg-ui --lib && cargo test -p twg-render --lib && cargo test -p twg-update --lib`
4. `TWG_BUILD_NUMBER=<naik> cargo build --release -p twg-desktop`
5. Jalankan skrip paket sesuai platform.
6. Publish manifest update ke VPS (bagian Auto-update).
7. Uji: ikon muncul di taskbar/dock/menu, tidak ada console/terminal, login
   Publik dan Lokal berfungsi, tombol **Keluar Aplikasi** menutup aplikasi,
   update dari build lama berhasil dan restart.
