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

## Daftar periksa rilis

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test -p twg-ui --lib && cargo test -p twg-render --lib`
4. `cargo build --release -p twg-desktop -p twg-online-admin`
5. Jalankan skrip paket sesuai platform.
6. Uji: ikon muncul di taskbar/dock/menu, tidak ada console/terminal, login
   Publik dan Lokal berfungsi, tombol **Keluar Aplikasi** menutup aplikasi.
