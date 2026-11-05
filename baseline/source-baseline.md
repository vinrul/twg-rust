# Source Baseline

**Sumber:** `E:\tactical_game`  
**Workspace migrasi:** `F:\twgrev`  
**Kebijakan:** source lama read-only

## Commit referensi

| Item | Nilai |
|---|---|
| Branch | `main` |
| Upstream | `origin/main` |
| Commit | `91d364d0cfc5f411c11160bc4e02717be73f3a05` |
| Tanggal | 2026-09-07 02:26:35 +08:00 |
| Subject | add hiace brimob dan timeout unlimited |
| Worktree | bersih saat analisis |

## Ukuran source

| Area | Jumlah |
|---|---:|
| File Go | 108 |
| Ukuran Go | 443.532 byte |
| File TypeScript/TSX | 411 |
| Ukuran TypeScript/TSX | 3.867.720 byte |
| Pasangan migrasi | 31 |
| File migrasi | 62 |
| Route backend | 111 |
| File aset | 580 |
| Ukuran aset | 217,61 MiB |
| GLTF/GLB | 515 |
| GLTF/GLB dengan Draco | 453 |
| GLTF/GLB dengan meshopt | 0 |

## Route per tingkat akses

| Akses | Jumlah |
|---|---:|
| Public | 19 |
| Bearer saja | 1 |
| Bearer + session | 62 |
| Admin role | 25 |
| Snapshot write role | 3 |
| Static publik | 1 |

Detail route ada di [api/routes.generated.md](api/routes.generated.md).

## Tooling saat analisis

| Tool | Status |
|---|---|
| Rust | `rustc 1.99.1` |
| Go | `go 1.26.10` |
| Node | `v24.22.2` |
| pnpm | tersedia |
| Docker CLI | tersedia (tidak dipakai) |
| Docker daemon | tidak dipakai |
| Podman client | `6.1.4` |
| Podman server | `6.0.2` |
| PostgreSQL image | `docker.io/library/postgres:18.4-alpine` |
| Centrifugo image | `docker.io/centrifugo/centrifugo:v6.2.5` |
| psql lokal | tidak tersedia |

## Status runtime

- Database fixture berjalan di container `twg-baseline-postgres` port `55432`.
- Server lama berjalan terhadap fixture dan sehat di port `8010`.
- Golden capture: **81 respons**, verifikasi **PASS**.
- Latency server lokal: lihat
  [golden/server-latency.generated.md](golden/server-latency.generated.md).
- Baseline FPS/memori renderer client belum diukur.

## Batas analisis

- Source tidak diubah.
- Server lama berhasil dibangun ke `baseline/.runtime/twg-baseline-server.exe`
  tanpa menulis ke source.
- Database fixture berjalan di Podman dengan PostgreSQL 18.4.
- Golden response: 81 respons, verifikasi PASS.
- WebSocket: Centrifugo v6.2.5, publish/subscribe PASS, cross-satker 403.
- Latency server lokal sudah diukur.
- Baseline FPS/memori renderer client belum diukur.
- Jangan menjalankan capture terhadap API produksi yang tertanam di source client.

## Artefak terkait

- [api/contract-notes.md](api/contract-notes.md)
- [api/websocket-baseline.md](api/websocket-baseline.md)
- [assets/assets-summary.generated.md](assets/assets-summary.generated.md)
- [security/findings.md](security/findings.md)
- [feature-parity-matrix.md](feature-parity-matrix.md)
- [phase0-report.md](phase0-report.md)
