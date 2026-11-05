# Feature Parity Matrix

Status awal Fase 0: semua fitur **belum dimigrasikan**. Matrix ini menjadi
checklist parity dan dasar penentuan scope.

| Domain | Fitur | Bukti source | Target Rust | Status |
|---|---|---|---|---|
| Auth | Login username/password | `server/api/users.go`, `3dmap/src/pages/auth/LoginPage.tsx` | `twg-auth`, `twg-server` | Belum |
| Auth | Refresh token | `axiosInstance.ts`, `users.go` | `twg-auth`, `twg-client` | Belum |
| Auth | Logout/revoke | `users.go`, `sessions/service.go` | `twg-auth`, `twg-server` | Belum |
| Auth | Device session | `middleware.go`, `sessions/service.go` | `twg-auth` | Belum |
| Auth | Single-device role | `SINGLE_DEVICE_ROLES` | `twg-auth` | Belum |
| Auth | Identify/heartbeat | `AuthProvider.tsx` | `twg-client` | Belum |
| Auth | Ganti password | `users.go`, `ProfileIndex.tsx` | `twg-server`, `twg-ui` | Belum |
| RBAC | Lima role | `middleware.go`, `use-role-flags.ts` | `twg-domain` | Belum |
| RBAC | Isolasi satker | handler/service/query | `twg-domain`, `twg-storage` | Belum |
| RBAC | Data tanpa satker untuk `user` | query dan handler | `twg-domain`, `twg-storage` | Belum |
| User | List/detail/create/update/delete | `/admin/users`, `/private/users` | `twg-server`, `twg-ui` | Belum |
| User | Session list | `/admin/users/:id/sessions` | `twg-server` | Belum |
| Satker | CRUD dan active flag | `/admin/satkers` | `twg-server`, `twg-ui` | Belum |
| Kategori | CRUD/pagination | `/admin/categories` | `twg-server`, `twg-ui` | Belum |
| Operasi | List/pagination/search | `/private/operasis` | `twg-server`, `twg-ui` | Belum |
| Operasi | Create/update/delete | `/admin/operasis` | `twg-server`, `twg-ui` | Belum |
| Operasi | Home list dengan skenario | `/public/list/operasis` | `twg-server` | Belum |
| Skenario | List/detail | `/public/skenarios/*` | `twg-server` | Belum |
| Skenario | Create/update/delete | `/admin/skenarios/*` | `twg-server`, `twg-ui` | Belum |
| Skenario | Copy dengan child entities | `/admin/skenarios/copy` | `twg-server`, transaksi | Belum |
| Skenario | Indoor config/floor plan | `SkenarioMapCreate.tsx`, migrasi 31 | `twg-map`, `twg-ui` | Belum |
| Peta | Tile OSM/satelit | `helpers/maps.ts` | `twg-map` | Belum |
| Peta | Tile proxy dan cache | belum ada | `twg-server`, `twg-map` | Belum |
| Peta | Zoom/pitch/bearing | `createMap` | `twg-map` | Belum |
| Peta | Indoor grid/blank | `createIndoorLayer` | `twg-map` | Belum |
| Peta | Floor plan overlay | `applyFloorPlanLayer` | `twg-map` | Belum |
| Peta | Ekspor gambar | `MapHome` toolbar | `twg-map`/`twg-client` | Belum |
| Geocode | Bali + Nominatim | `api/geocode.ts` | `twg-client` via server | Belum |
| Marker | CRUD | `/private/markers/*` | `twg-server`, `twg-map` | Belum |
| Marker | GLTF/unit catalog | `utils/items.ts`, `utils/models.ts` | `twg-render` | Belum |
| Marker | Rotasi/skala/altitude | marker DTO/handler | `twg-map` | Belum |
| Marker | Duplicate | `use-marker-transform` | `twg-map`, `twg-ui` | Belum |
| Marker | Selection dan context menu | `marker-manager.ts` | `twg-map`, `twg-ui` | Belum |
| Marker | Transform gizmo | `transform/*` | `twg-map`, `twg-ui` | Belum |
| Building | CRUD/geom | `/private/buildings/*` | `twg-server`, `twg-map` | Belum |
| Building | Extrude polygon | `objects.ts` | `twg-render` | Belum |
| Line | CRUD/geom | `/private/lines/*` | `twg-server`, `twg-map` | Belum |
| Line | Draw/edit/shift | `draw-tool*`, `use-line-transform` | `twg-map`, `twg-ui` | Belum |
| Text marker | CRUD/visibility | `/private/text-markers/*` | `twg-server`, `twg-map` | Belum |
| Layer | Panel/visibility | `LayerPanel.tsx` | `twg-ui` | Belum |
| Editor | Toolbar plot | `map-ui-manager.ts` | `twg-ui` | Belum |
| Editor | Undo/redo command bus | `command-bus.ts` | `twg-client` | Belum |
| Editor | Measure/grid/snap | TODO project | `twg-map`, `twg-ui` | Belum |
| Anim | Route | `anim-*.ts`, `/anims/route` | `twg-render`, `twg-server` | Belum |
| Anim | Spawn/remove/combine | anim handlers | `twg-render`, `twg-server` | Belum |
| Anim | Sequence/delay | `anim-sequence.ts` | `twg-render` | Belum |
| Anim | Camera follow/zoom | `camera-follow-*` | `twg-render` | Belum |
| Anim | Rotasi/skala/3D | `anim-rot-scale.ts` | `twg-render` | Belum |
| Anim | Vehicle/traffic/patroli/parade | berbagai `anim-*.ts` | `twg-render` | Belum |
| Anim | Unit flow/handoff | `anim-unit-flow.ts` | `twg-render` | Belum |
| Efek | Particle | `particles/*`, `/particles` | `twg-render` | Belum |
| Efek | Shoot | `games/effects.ts` | `twg-render` | Belum |
| Efek | Video tron | `video-tron.ts` | `twg-render` | Belum |
| Audio | Sound marker/anim | `utils/sounds.ts` | `twg-render` | Belum |
| Runtime | Preview/game/training | `PreviewGame.tsx`, `PreviewMap.tsx` | `twg-client`, `twg-render` | Belum |
| Runtime | Last position/last map | `/last_positions` | `twg-server`, `twg-client` | Belum |
| Runtime | Snapshot/resume | `/game-sessions/snapshot` | `twg-server`, `twg-client` | Belum |
| Runtime | AAR/replay | TODO project | belum diputuskan | Belum |
| Realtime | Publish command | `/private/socket` | `twg-server` WS | Belum |
| Realtime | Presence/online user | `/public/users/online` | `twg-server` WS | Belum |
| Realtime | Reconnect/resume | belum ada eksplisit | `twg-client` WS | Belum |
| Upload | Image | `/private/upload-image` | `twg-server`, `twg-ui` | Belum |
| Upload | Video | `/private/upload-video` | `twg-server`, `twg-ui` | Belum |
| UI | Login/layout/navbar | React/shadcn | `twg-ui` | Belum |
| UI | Tabel/pagination | React table | `egui_extras` | Belum |
| UI | Dialog/sheet/tooltip | Radix/shadcn | `egui`/`egui_dock` | Belum |
| UI | Toast | sonner | `egui-notify` | Belum |
| UI | Drag-drop | dnd-kit | `egui_dnd` | Belum |
| UI | Form/validation | react-hook-form/Zod | egui + validator | Belum |
| Platform | Native Windows/Linux/macOS | belum ada | Bevy | Belum |
| Platform | Web WASM | React web sekarang | Bevy WASM | Belum |
| Platform | Offline/PMTiles | belum ada | belum diputuskan | Belum |

## Prioritas rilis pertama yang disarankan

Wajib:

- Auth/session/RBAC.
- Operasi/skenario CRUD.
- Peta raster outdoor + kamera.
- Marker GLTF + line + building.
- Simpan/muat skenario.
- Snapshot/last position.
- Satu alur animasi inti (route + spawn/remove).
- Realtime command dasar.

Dapat ditunda jika gate peta/WebGL2 bermasalah:

- Video tron.
- Partikel GPU.
- 3D terrain/globe.
- AAR/replay.
- Measure/grid/snap.
