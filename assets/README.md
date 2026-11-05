# Sample assets

`models/motor_patroli_brimob_anim_1.glb` is copied from the read-only source
project at
`E:\tactical_game\3dmap\public\kendaraans\motor_patroli_brimob_anim_1.glb` for
the Phase 3 renderer spike. It is a non-Draco GLB with two animation clips and a
valid embedded JPEG texture.

The source project remains read-only. This directory is the new workspace asset
root used by the desktop binary and copied into the Trunk bundle for web.

## Marker models

| File | Triangles | Animation | Purpose |
|---|---:|---|---|
| `polwan_airud_walk_non_draco.glb` | 9.628 | walk | Full-detail animated marker |
| `polwan_airud_idle_non_draco.glb` | 9.628 | — | Full-detail static marker |
| `polwan_airud_walk_draco.glb` | 9.628 | walk | Draco probe (tidak didukung Bevy 0.20) |
| `polwan_airud_idle_draco.glb` | 9.628 | — | Draco probe (tidak didukung Bevy 0.20) |
| `polwan_airud_walk_lod2.glb` | 2.404 | walk | Web/LOD stress and production proxy |
| `polwan_airud_idle_lod2.glb` | 2.406 | — | Web/LOD stress and production proxy |

`lod2` files were produced from the non-Draco originals with:

```bash
npx --yes @gltf-transform/cli@4.5.1 weld \
  assets/models/polwan_airud_walk_non_draco.glb target/lod/walk-weld.glb
npx --yes @gltf-transform/cli@4.5.1 simplify \
  target/lod/walk-weld.glb target/lod/walk-simple.glb --ratio 0.25 --error 0.01
```

The skeleton, skin weights, and the `Idle|Walk` animation clip are preserved.
WebGL2 saturates at roughly one billion triangles per second on the reference
GTX 1660 Ti, so the full-detail mesh cannot keep 10.000 markers at 25-30 FPS.
The LOD keeps the same animation and material while cutting triangle load by
about 4x.

## Draco

Bevy 0.20 does not implement `KHR_draco_mesh_compression`. The two `*_draco`
files are kept only for the `--probe-draco` check, which reports FAIL with
`Unsupported extension`. The desktop and web clients use the non-Draco and
`lod2` files.
