# Laporan Fase 1 — Workspace Rust dan CI

**Tanggal:** 4 November 2026  
**Status:** selesai; Trunk juga sudah terpasang dan `trunk build` berhasil.

## 1. Yang dibangun

- Cargo workspace dengan 13 package.
- 10 crate library dan 3 binary/app:
  - `twg-domain`
  - `twg-contracts`
  - `twg-auth`
  - `twg-storage`
  - `twg-server`
  - `twg-client`
  - `twg-map`
  - `twg-render`
  - `twg-ui`
  - `twg-asset-pipeline`
  - `twg-server-app`
  - `twg-desktop`
  - `twg-web`
- Server Axum dengan `/healthz` dan `/readyz`.
- Desktop Bevy 0.20 + bevy_egui 0.43.
- WebAssembly target untuk `twg-web`.
- CI GitHub Actions untuk format, clippy, test, WASM, deny, dan audit.
- `deny.toml`, `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`.

## 2. Versi utama

| Dependency | Versi |
|---|---|
| Rust | `1.99.1` |
| Bevy | `0.20.1` |
| bevy_egui | `0.43.1` |
| egui (transitif) | `0.36.2` |
| Axum | `0.8.9` |
| Tokio | `1.54.1` |
| SQLx | `0.9.0` |
| PostgreSQL feature | `postgres`, `runtime-tokio`, `uuid`, `chrono`, `json`, `migrate` |

## 3. Aturan dependency

Diverifikasi dengan `cargo tree`:

| Aturan | Hasil |
|---|---|
| `twg-server` tidak mengimpor Bevy | PASS |
| `twg-server` memakai Axum | PASS |
| `twg-domain` tidak memakai Axum/SQLx/Bevy | PASS |
| `twg-web` tidak memakai Axum/SQLx | PASS |
| Client dan server berbagi `twg-contracts` | PASS |

## 4. Verifikasi

| Pemeriksaan | Hasil |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| Test crate inti + server | **21 tests PASS** |
| `cargo check -p twg-web --target wasm32-unknown-unknown` | PASS |
| `cargo build -p twg-web --target wasm32-unknown-unknown` | PASS |
| `trunk build` di `apps/web` | PASS |
| `cargo build -p twg-server-app` | PASS |
| `cargo build -p twg-desktop` | PASS |
| Server health/ready | HTTP 200 |
| Desktop runtime smoke test | 12 detik, tidak panic |
| `cargo deny check` | PASS |
| `cargo audit` | PASS — advisory transitif di-ignore eksplisit |

### Rincian test

| Crate | Test |
|---|---:|
| `twg-auth` | 3 |
| `twg-client` | 2 |
| `twg-contracts` | 4 |
| `twg-domain` | 5 |
| `twg-map` | 5 |
| `twg-server` | 2 |
| **Total** | **21** |

## 5. Artefak build

| Artefak | Ukuran |
|---|---:|
| `target/debug/twg-server-app.exe` | 2,69 MB |
| `target/debug/twg-desktop.exe` | 120,56 MB |
| `target/wasm32-unknown-unknown/debug/twg_web.wasm` | 1.511 MB |
| `apps/web/dist/*_bg.wasm` hasil Trunk | 154,50 MB |

WASM debug sangat besar karena debug info. Profil `wasm-release` sudah disiapkan
dengan `opt-level = "s"`, LTO, dan strip. Ukuran release akan diukur pada Fase 3
atau saat pipeline web disiapkan.

## 6. Masalah yang ditemukan dan diperbaiki

1. `DirectionalLight.shadows_enabled` berubah menjadi `shadow_maps_enabled` di
   Bevy 0.20.
2. Sistem egui harus dijadwalkan pada `EguiPrimaryContextPass`, bukan `Update`.
   Sebelum diperbaiki, desktop panic dengan `No fonts available`.
3. Asersi floating-point Web Mercator terlalu ketat; diganti toleransi `1e-9`.
4. `twg-domain` sempat kehilangan manifest saat menambah dev-dependency; sudah
   dipulihkan.
5. `cargo deny` membutuhkan lisensi `MIT-0` dan ignore advisory unmaintained
   `RUSTSEC-2026-0192` (`ttf-parser` transitif dari winit).

## 7. Audit dan supply chain

- `cargo deny check`: **advisories ok, bans ok, licenses ok, sources ok**.
- Warning duplicate versions berasal dari dependency Bevy (base64, bitflags,
  dst.) dan tidak menggagalkan kebijakan.
- `cargo audit` melaporkan `ttf-parser 0.25.1` unmaintained melalui
  `winit -> sctk-adwaita -> ab_glyph`. Tidak ada safe upgrade saat ini.
- Advisory di-ignore secara eksplisit di `deny.toml` dan `.cargo/audit.toml`
  sampai winit/Bevy mengganti dependency tersebut.

## 8. Yang belum selesai

1. **GitHub Actions belum dijalankan di remote** karena workspace belum memiliki
   remote repository. Workflow lokal sudah divalidasi dengan perintah yang sama.
2. **Fitur client/server belum diimplementasikan**; ini masih scaffold Fase 1.
3. **Ukuran WASM release** belum diukur. Bundle Trunk debug saat ini 154,5 MB;
   profil `wasm-release` belum dipakai oleh Trunk.
4. **Bevy default features** masih penuh (audio, gamepad, dll.). Optimasi fitur
   dilakukan saat renderer `twg-map` mulai dibangun.

Trunk 0.22.1 sudah terpasang dan `trunk build` di `apps/web` berhasil
menghasilkan `index.html`, JS glue, dan WASM.

## 9. Exit gate Fase 1

| Gate | Status |
|---|---|
| Satu perintah build server | PASS — `cargo build -p twg-server-app` |
| Satu perintah build desktop | PASS — `cargo build -p twg-desktop` |
| Satu perintah build web | PASS — Cargo/WASM dan `trunk build` |
| CI native + WASM tersedia | PASS secara konfigurasi; remote pending |
| CI lokal hijau | PASS untuk fmt, clippy, test, WASM, deny |
| Tidak ada dependency client di server | PASS |

## 10. Langkah berikutnya

Masuk Fase 2: kontrak domain dan DTO berdasarkan golden response Fase 0.
Fokus pertama:

1. DTO envelope/auth/operasi/skenario.
2. Enum error dan pemetaan status.
3. `units` string/array adapter.
4. Snapshot test terhadap `baseline/golden/redacted`.
5. Feature parity matrix sebagai daftar kerja.
