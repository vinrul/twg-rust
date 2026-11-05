# Baseline Fixtures

Fixture ini hanya untuk database lokal dan tidak boleh dipakai di produksi.

## Isi

- `docker-compose.yml` — alternatif Compose untuk PostgreSQL 18.4 di port `55432`.
- `bootstrap.sql` — guard database test, reset schema, dan ekstensi `uuid-ossp`.
- `seed.sql` — data uji enam akun, dua satker, tiga operasi, empat skenario,
  marker, building, line, text marker, particle, anim, command, last map,
  last position, dan snapshot.
- `migrations/` — salinan read-only dari 31 migrasi `up.sql` source lama.
- `app.env.baseline` — konfigurasi server test.
- `centrifugo.json` — konfigurasi Centrifugo v6 lokal. `client.insecure` hanya
  untuk development/test; API key lokal `baseline-only-not-secret` bukan secret
  produksi.

Runner utama memakai **Podman native**, image PostgreSQL
`docker.io/library/postgres:18.4-alpine`, dan image Centrifugo
`docker.io/centrifugo/centrifugo:v6.2.5`. PostgreSQL 18 membutuhkan volume di
`/var/lib/postgresql`, bukan `/var/lib/postgresql/data`.

## Akun

Semua password: `Baseline#2026`

| Username | Role | Satker |
|---|---|---|
| `baselineadmin` | `admin` | — |
| `baselinemonitor` | `admin_monitor` | — |
| `baselineadminsatker` | `admin_satker` | Satker Alpha |
| `baselineusersatker` | `user_satker` | Satker Alpha |
| `baselineuser` | `user` | — |
| `baselineuserbeta` | `user_satker` | Satker Beta |

## ID utama

| Entity | ID |
|---|---|
| Satker Alpha | `11111111-1111-4111-8111-111111111111` |
| Satker Beta | `22222222-2222-4222-8222-222222222222` |
| Operasi Alpha | `33333333-3333-4333-8333-333333333331` |
| Skenario Alpha | `44444444-4444-4444-8444-444444444441` |
| Skenario Indoor Alpha | `44444444-4444-4444-8444-444444444443` |
| Marker Alpha | `55555555-5555-4555-8555-555555555551` |
| Building Alpha | `66666666-6666-4666-8666-666666666661` |
| Line Alpha | `77777777-7777-4777-8777-777777777771` |
| Text Marker Alpha | `88888888-8888-4888-8888-888888888881` |
| Particle Alpha | `99999999-9999-4999-8999-999999999991` |
| Anim Route Alpha | `bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1` |
| Snapshot Alpha | `ffffffff-ffff-4fff-8fff-fffffffffff1` |

## Menjalankan

Podman engine harus aktif.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-fixture-db.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-baseline-server.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-baseline-centrifugo.ps1
```

Lalu di terminal lain:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/capture-golden.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-golden.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/capture-websocket.ps1
```

Untuk mengulang dari database kosong:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-fixture-db.ps1 -Reset
```

`run-fixture-db.ps1` menghapus container test tanpa menghapus volume, lalu
me-reset schema `public` secara deterministik. Gunakan `-Reset` untuk menghapus
volume juga.

## Keamanan

- Kredensial di sini hanya untuk localhost.
- Jangan mengarahkan `DB_SOURCE` ke database produksi.
- `seed.sql` menolak berjalan jika nama database bukan `twg_baseline`.
- Script capture menolak host non-local kecuali diberi `-AllowRemote`.
