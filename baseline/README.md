# Baseline Migrasi Rust

Folder ini menyimpan baseline read-only dari proyek lama dan alat untuk
memverifikasi kontrak sebelum implementasi Rust.

## Guardrail

- Source lama: `E:\tactical_game` — **jangan diubah**.
- Workspace migrasi: `F:\twgrev`.
- Semua capture runtime harus diarahkan ke instance test lokal.
- Jangan menjalankan script capture ke production.
- Secret/API key yang ditemukan di source lama tidak boleh disalin.

## Commit referensi

`91d364d0cfc5f411c11160bc4e02717be73f3a05`  
Branch `main`, commit 2026-09-07, worktree bersih.

## Isi

| Path | Fungsi |
|---|---|
| `source-baseline.md` | Ringkasan commit, ukuran source, tooling |
| `source-baseline.json` | Metadata baseline terstruktur |
| `phase0-report.md` | Status Fase 0, blocker, dan langkah penutupan |
| `feature-parity-matrix.md` | Checklist parity fitur lama → Rust |
| `api/routes.generated.md` | 111 route backend |
| `api/contract-notes.md` | Envelope, auth, pagination, upload, risiko |
| `api/websocket-baseline.md` | Command dan payload real-time |
| `assets/assets-summary.generated.md` | 580 aset, 453 GLTF memakai Draco |
| `security/findings.md` | Temuan keamanan baseline |
| `fixtures/` | PostgreSQL fixture, seed, akun test |
| `scripts/` | Generator inventaris, DB fixture, server, capture |
| `golden/` | Output golden response setelah server test berjalan |

## Alur verifikasi

```powershell
# 1. Jalankan database fixture (Podman/Docker engine harus aktif)
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\run-fixture-db.ps1

# 2. Jalankan server lama dengan konfigurasi test
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\run-baseline-server.ps1

# 3. Jalankan Centrifugo lokal
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\run-baseline-centrifugo.ps1

# 4. Di terminal lain, ambil dan verifikasi golden response
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\capture-golden.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\verify-golden.ps1

# 5. Uji WebSocket end-to-end
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\capture-websocket.ps1

# 6. Ukur latency server lokal
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\measure-server-baseline.ps1
```

## Regenerasi inventaris

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\extract-routes.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\inventory-assets.ps1
```

## Status

- Artefak statis: selesai.
- Build server lama: berhasil.
- Database fixture Podman/PostgreSQL 18.4: berjalan.
- Golden response: **81 respons**, verifikasi **PASS**.
- WebSocket: **Centrifugo v6.2.5**, publish/subscribe **PASS**, cross-satker 403.
- Latency server lokal: terukur.
- Baseline FPS/memori renderer client: belum diukur.
- MapTiler tidak dipakai; aplikasi OSM-only dan tidak membutuhkan API key.
- Dead code MapTiler perlu dihapus dari source baru.
