# Laporan Fase 0

**Tanggal:** 4 November 2026  
**Status:** artefak dan verifikasi runtime dasar selesai; baseline renderer client belum diukur.

## 1. Keputusan workspace

- Workspace migrasi: `F:\twgrev`.
- Baseline: `F:\twgrev\baseline`.
- Source lama: `E:\tactical_game` — read-only.
- Workspace Rust final akan menggunakan root `F:\twgrev` pada Fase 1.
- Source lama tidak diubah, dibuat, atau dihapus.

## 2. Baseline source

- Commit: `91d364d0cfc5f411c11160bc4e02717be73f3a05`.
- Branch: `main`, upstream `origin/main`.
- Worktree bersih.
- 108 file Go; 411 file TS/TSX.
- 31 pasang migrasi.
- 111 route backend.
- 580 aset; 515 GLTF/GLB; 453 memakai Draco.
- Detail: [source-baseline.md](source-baseline.md), [source-baseline.json](source-baseline.json).

## 3. Status checklist Fase 0

### 3.1 Lokasi dan guardrail

| Item | Status | Bukti |
|---|---|---|
| Tentukan workspace baru | Selesai | `F:\twgrev` |
| Pastikan tidak di dalam source lama | Selesai | path terpisah |
| Catat commit source | Selesai | `source-baseline.json` |
| README source read-only | Selesai | `baseline/README.md` |
| `.gitignore` | Selesai | `baseline/.gitignore` |
| Strategi branch | Selesai | `main` stabil + branch per fase |
| Otoritas approval | Selesai | pemilik proyek |
| Build server lama tanpa mengubah source | Selesai | `baseline/.runtime/twg-baseline-server.exe` |

### 3.2 Baseline perilaku dan data

| Item | Status | Catatan |
|---|---|---|
| Inventaris route | Selesai | 111 route generated |
| Method/path/access/handler | Selesai | `api/routes.generated.*` |
| Request/response detail | Selesai | 81 golden response; request CRUD/large upload belum |
| Fixture database | Selesai | `fixtures/seed.sql` |
| Database fixture berjalan | Selesai | Podman, PostgreSQL 18.4 |
| Pengguna lima role | Selesai | enam akun termasuk cross-satker |
| Data uji per role | Selesai | global, alpha, beta, indoor |
| Golden response | Selesai | 81 respons, `golden/` |
| Error response | Selesai | 403/400 body tersimpan |
| Pagination | Selesai | 1-based, offset formula |
| Upload behavior | Selesai | kontrak + temuan keamanan |
| Refresh/revoke/logout/heartbeat | Sebagian | refresh/login/identify tervalidasi |
| `X-Device-ID` | Selesai | body/header, header prioritas |
| WebSocket messages | Sebagian | command dan payload terdokumentasi |
| WebSocket publish/subscribe | Selesai | Centrifugo v6.2.5, payload diterima |
| WebSocket cross-satker | Selesai | publish lintas satker ditolak 403 |
| Reconnect/snapshot runtime | Tertunda | butuh skenario reconnect khusus |
| `units` dual shape | Selesai | dikonfirmasi golden |
| Timestamp/null/UUID/angka | Selesai | dikonfirmasi golden |
| Feature parity matrix | Selesai | `feature-parity-matrix.md` |

### 3.3 Baseline performa dan aset

| Item | Status | Catatan |
|---|---|---|
| Ukuran source | Selesai | Go/TS/asset |
| Inventaris aset | Selesai | 580 file |
| Distribusi tipe aset | Selesai | `assets-summary.generated.md` |
| Draco | Selesai | 453/515 GLTF memakai Draco |
| Provider tile/attribution | Selesai | OSM utama; Esri satelit opsional; MapTiler dead code |
| API key tile | Tidak diperlukan | Aplikasi memakai OSM tanpa key |
| Dead code MapTiler | **Bersihkan** | fungsi tidak dipakai; hapus, bukan blocker |
| Rencana tile proxy | Selesai | untuk cache/rate limit OSM, bukan key |
| Latency server lokal | Selesai | `golden/server-latency.generated.md` |
| FPS/memori/load time client | **Tertunda** | butuh client + browser |
| Bundle size produksi | **Tertunda** | belum build client lama |

### 3.4 Temuan keamanan

| Item | Status |
|---|---|
| Inventaris secret | Selesai |
| Token localStorage | Selesai |
| Public kick-session | Selesai — kritis |
| Public online users | Selesai — kritis |
| Global real-time channel | Selesai — tinggi |
| Upload validation | Selesai — tinggi |
| Rate limit | Selesai — tinggi |
| CORS/CSRF | Selesai — perlu review |
| Secret tidak disalin | Selesai |

Detail: [security/findings.md](security/findings.md).

## 4. Blocker yang tersisa

1. **Baseline FPS/memori renderer client belum diukur.** Ini membutuhkan
   instance client yang aman dan tidak boleh diarahkan ke API/Centrifugo
   production yang tertanam di source.
2. **Bundle size produksi belum diukur** karena build client lama belum
   dijalankan di workspace terisolasi.
3. **Dead code MapTiler** perlu dihapus dari source baru; tidak perlu API key
   untuk OSM.
4. **Keputusan produk** tentang endpoint publik yang mengekspos data operasi.
5. **Keputusan sesi lama** saat cutover belum diambil.
6. **Target WebGL2 vs WebGPU** belum difinalkan; memengaruhi Fase 3/10.

Database, golden response, role matrix, error body, latency server, dan
WebSocket publish/subscribe sudah terverifikasi. Tidak ada blocker untuk
memulai Fase 1.

## 5. Artefak yang dihasilkan

```text
baseline/
├── README.md
├── .gitignore
├── source-baseline.json
├── source-baseline.md
├── phase0-report.md
├── feature-parity-matrix.md
├── api/
│   ├── contract-notes.md
│   ├── websocket-baseline.md
│   ├── routes.generated.csv
│   └── routes.generated.md
├── assets/
│   ├── assets.generated.csv
│   └── assets-summary.generated.md
├── fixtures/
│   ├── README.md
│   ├── docker-compose.yml
│   ├── bootstrap.sql
│   ├── seed.sql
│   ├── app.env.baseline
│   ├── centrifugo.json
│   └── migrations/ (31 up.sql)
├── golden/
│   ├── README.md
│   ├── capture-meta.json
│   ├── capture-summary.generated.md
│   ├── verification.generated.md
│   ├── server-latency.generated.md
│   ├── websocket-capture.json
│   ├── websocket-capture.generated.md
│   ├── redacted/ (81 respons)
│   └── raw/ (diabaikan Git)
├── security/
│   └── findings.md
└── scripts/
    ├── extract-routes.ps1
    ├── inventory-assets.ps1
    ├── run-fixture-db.ps1
    ├── run-baseline-server.ps1
    ├── run-baseline-centrifugo.ps1
    ├── capture-golden.ps1
    ├── verify-golden.ps1
    ├── capture-websocket.js
    ├── capture-websocket.ps1
    ├── measure-server-baseline.ps1
    ├── stop-baseline-containers.ps1
    └── hash-bcrypt/
```

## 6. Langkah untuk mengulang verifikasi

1. Pastikan Podman engine aktif.
2. Jalankan:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\run-fixture-db.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\run-baseline-server.ps1
```

3. Di terminal lain:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\capture-golden.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\verify-golden.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\capture-websocket.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\measure-server-baseline.ps1
```

Centrifugo dijalankan dengan:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File F:\twgrev\baseline\scripts\run-baseline-centrifugo.ps1
```

4. Periksa `baseline/golden/verification.generated.md`.
5. Ukur baseline client lama di environment terisolasi, bukan production.
6. Hapus dead code MapTiler dan isi keputusan produk.

## 7. Rekomendasi sebelum Fase 1

Fase 1 (workspace Rust) boleh dimulai karena tidak bergantung pada runtime
database. Golden response sudah tersedia untuk Fase 2. Fase 3 spike peta juga
boleh dimulai paralel setelah workspace Rust tersedia.

Urutan aman:

1. Mulai Fase 1 workspace/CI.
2. Gunakan golden response untuk Fase 2.
3. Mulai spike `twg-map` paralel dengan server auth.
4. Ukur performa client lama saat environment terisolasi tersedia.
