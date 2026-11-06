# Laporan Fase 9 — Map core dan editor

Status: **selesai untuk alur editor utama** di desktop dan web. Item yang belum
setara rencana awal dicatat di [Deviasi](#deviasi).

## 1. Cakupan

Fase 9 mengubah layar Peta menjadi editor bergaya Godot/Unity:

- Menu bar `File / Edit / View / Help`.
- Toolbar tool dan aksi: Select, Move, Rotate, Scale, Marker, Line, Polygon,
  Text, Measure, Snap, Indoor, Simpan, Play, Kembali.
- Dock kiri: hierarki **Scene** per tipe objek, checkbox visibilitas, panel
  **Layers**, dan pencarian **Geocode**.
- Viewport peta 3D dengan pin objek, label, selection outline, transform gizmo,
  crosshair, kompas, scale bar, kontrol zoom/bearing/pitch, dan attribution.
- Dock kanan: **Inspector** skenario atau objek (transform, rotasi, skala,
  altitude, warna, teks, build height).
- Dock bawah: katalog unit **Assets** dan **Console**.
- Status bar: skenario, cursor, zoom/tile zoom, m/px, bearing/pitch, FPS,
  jumlah objek/selection, status simpan.
- Mode **Play** menyembunyikan dock dan menonaktifkan editing.

## 2. Struktur

```text
crates/twg-render/src/editor/
  mod.rs        EditorRenderPlugin, export tipe snapshot
  scene.rs      EditorScene/EditorMode, sinkronisasi entity, gizmo, grid,
                floor plan, marker pins 3D, update skala gizmo
  mesh.rs       ribbon, prism, quad, grid, circle, cuboid, cone, sphere

crates/twg-ui/src/editor/
  mod.rs        export modul editor
  model.rs      EditorState, objek, tool, katalog unit, undo/redo, snap,
                build_scene (snapshot ke renderer), tes unit
  api.rs        load/save objek lewat REST, job async, geocode
  input.rs      picking, marquee, drag objek, gizmo, keyboard, measure,
                draft line/polygon
  panels.rs     layout dock, inspector, assets/console, overlay viewport,
                context menu, dialog perubahan belum disimpan
```

`twg-render` tetap tidak bergantung pada UI. `twg-ui` mengirim snapshot data
ke resource `EditorScene`; renderer hanya membuat entity dari snapshot itu.

## 3. Interaksi

| Aksi | Kontrol |
|---|---|
| Pilih tool | `Q` Select, `W` Move, `E` Rotate, `R` Scale |
| Buat objek | `1` Marker, `2` Line, `3` Polygon, `4` Text, `5` Measure |
| Pilih objek | Klik pin/garis/bangunan; Ctrl+klik untuk multi-select |
| Marquee | Drag di area kosong |
| Pindah objek | Drag objek terpilih atau axis gizmo |
| Rotasi | Tool Rotate + drag ring kuning |
| Skala | Tool Scale + drag axis |
| Selesai line/polygon | `Enter` atau klik ganda |
| Batal | `Esc` atau klik kanan saat menggambar |
| Hapus / duplikat | `Del` / `Ctrl+D` |
| Undo / redo | `Ctrl+Z` / `Ctrl+Y` |
| Simpan | `Ctrl+S` atau tombol Simpan |
| Snap | Combo Snap (Off/1/5/10/25 m) atau `G` |
| Pan/orbit/zoom | Drag tengah/kanan, wheel |

Klik kanan tanpa drag pada objek membuka context menu Duplikat, Hapus, Fokus.

## 4. Penyimpanan

Saat skenario dibuka, editor memuat detail skenario dan lima daftar objek
(`markers`, `lines`, `buildings`, `text-markers`, `particles`) dari endpoint
publik. Marker memakai katalog unit lokal dengan UUID tetap.

Saat Simpan, editor mengirim hanya perubahan:

- objek baru: `POST` endpoint masing-masing;
- objek berubah: `PUT` geom/nama/skala/altitude/teks sesuai tipe;
- objek terhapus: `DELETE` berdasarkan UUID server.

`MarkerCreateRequest` tidak membawa skala/altitude, jadi marker baru dengan
nilai non-default mengirim dua `PUT` tambahan setelah `POST` berhasil.

Undo/redo menyimpan snapshot objek + daftar hapus tertunda. Objek yang dibuat
lalu di-undo otomatis masuk daftar hapus server; objek yang dihapus lalu
di-undo dikeluarkan dari daftar itu. Ini menjaga data server tetap konsisten
setelah siklus undo/save.

Perubahan yang belum disimpan memunculkan konfirmasi saat Kembali, Muat ulang,
atau menutup editor.

## 5. Peta dan indoor

- Raster OSM/Satelit tetap memakai pipeline Fase 3 (cache, proxy, overzoom
  provider z19).
- Attribution provider digambar di kanan bawah viewport.
- Mode Indoor menyembunyikan raster dan menampilkan grid 1 m (bisa diubah)
  serta floor plan prosedural.
- `min_zoom`/`max_zoom` mengikuti skenario; kamera bisa sampai zoom 26.
- Kamera skenario (center, zoom, pitch, bearing) dipulihkan lewat fly-to.

## 6. Geocode

Server menambah `GET /public/geocode?q=...` yang mem-proxy OpenStreetMap
Nominatim (limit 6 hasil, `User-Agent` khusus). UI menampilkan hasil dan
mem-fly kamera ke lokasi yang dipilih. Endpoint diuji langsung:

```text
GET /public/geocode?q=Denpasar
200 {"data":[{"label":"Denpasar, Bali, Indonesia","latitude":-8.6653349,...}]}
```

## 7. Bukti uji

Perintah:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p twg-client -p twg-render -p twg-ui -p twg-web \
  --target wasm32-unknown-unknown -- -D warnings
cargo test --workspace
cd apps/web && trunk build --cargo-profile wasm-release
node tools/client-smoke.mjs --username=baselinemonitor \
  --scenario=44444444-4444-4444-8444-444444444441 --editor-test=true
```

Hasil:

- 8 tes unit editor PASS (tambah/undo/redo, duplikat, hapus + antrean delete
  server, undo delete, gizmo, layer visibility, snap, warna hex).
- Seluruh tes workspace PASS; clippy native dan WASM bersih.
- Smoke editor web PASS:
  - marker, line, building, dan text dibuat lewat toolbar;
  - `POST /private/markers` 201 dan endpoint line/building/text tersimpan;
  - jumlah objek di server bertambah tepat satu per tipe;
  - halaman di-reload dan jumlah objek tetap;
  - marker baru dihapus lewat Inspector dan `DELETE` 200;
  - data uji dibersihkan kembali ke baseline.
- Geocode server 200 dengan hasil Nominatim.
- Probe Draco desktop 8/8 PASS setelah plugin editor ditambahkan.
- Screenshot: [`phase9-editor-create.png`](assets/phase9-editor-create.png)
  dan [`phase9-editor-reload.png`](assets/phase9-editor-reload.png).

## 8. Deviasi

| Item | Status | Catatan |
|---|---|---|
| Edit vertex line/polygon | Belum | Line/building bisa dibuat, dipindah, diduplikasi, dan dihapus sebagai satu objek; belum ada mode edit titik per vertex |
| Ekspor gambar peta | Belum | Tidak ada tombol screenshot/export di Fase 9 |
| Indicator error tile | Belum | Tile gagal tetap memakai material placeholder; belum ada hitungan error di UI |
| Floor plan nyata | Parsial | Overlay memakai grid prosedural; URL floor plan dari `indoor_config` belum dimuat sebagai gambar |
| Geocode offline | Catatan | Bergantung pada akses internet ke Nominatim |
| Uji desktop interaktif | Parsial | Desktop dibangun dan probe berjalan; interaksi editor diuji otomatis di web |

## 9. Langkah berikutnya

Fase 10 — animasi, runtime, dan efek: timeline internal, route movement,
spawn/remove/combine/sequence, shoot, particle, serta integrasi animasi ke
skenario yang disimpan.
