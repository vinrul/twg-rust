# Sample assets

`models/motor_patroli_brimob_anim_1.glb` is copied from the read-only source
project at
`E:\tactical_game\3dmap\public\kendaraans\motor_patroli_brimob_anim_1.glb` for
the Phase 3 renderer spike. It is a non-Draco GLB with two animation clips and a
valid embedded JPEG texture.

The source project remains read-only. This directory is the new workspace asset
root used by the desktop binary and copied into the Trunk bundle for web.

## Marker models

| File | Size | Triangles | Animation | Purpose |
|---|---:|---:|---|---|
| `polwan_airud_walk_non_draco.glb` | 711 KB | 9.628 | walk | Full-detail animated marker |
| `polwan_airud_idle_non_draco.glb` | 336 KB | 9.628 | — | Full-detail static marker |
| `polwan_airud_walk_draco.glb` | 251 KB | 9.628 | walk | Full-detail animated marker, Draco |
| `polwan_airud_idle_draco.glb` | 82 KB | 9.628 | — | Full-detail static marker, Draco |
| `polwan_airud_walk_lod2.glb` | 304 KB | 2.404 | walk | Web/LOD stress and production proxy |
| `polwan_airud_idle_lod2.glb` | 113 KB | 2.406 | — | Web/LOD stress and production proxy |
| `polwan_airud_walk_lod2_draco.glb` | 204 KB | 2.404 | walk | LOD proxy, Draco |
| `polwan_airud_idle_lod2_draco.glb` | 52 KB | 2.406 | — | LOD proxy, Draco |

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

The `*_lod2_draco.glb` files were produced from the `lod2` originals with:

```bash
npx --yes @gltf-transform/cli@4.5.1 draco \
  assets/models/polwan_airud_walk_lod2.glb \
  assets/models/polwan_airud_walk_lod2_draco.glb
```

Draco keeps the triangle count, skin weights, and animation clip; it only
shrinks the binary payload. The decoded vertex count can grow by a handful of
vertices because quantization splits a few shared vertices.

## Draco

Bevy 0.20 does not implement `KHR_draco_mesh_compression` itself, so the
workspace vendors the upstream `bevy_gltf_draco` plugin as
`crates/twg-gltf-draco` (see its README). Draco files must be loaded with
`GltfLoaderSettings::validate = false`; `crates/twg-render/src/stress/gltf.rs`
does that for the `*_draco.glb` paths.

`--probe-draco` loads all eight marker models (plain and Draco, full and LOD).
Use `--draco` on the desktop client or `?draco=1` on the web client to run the
stress scenarios against the compressed variants. Draco reduces download size
and decode is a one-off cost, so it does not change steady-state FPS; see
[`docs/DRACO_REPORT.md`](../docs/DRACO_REPORT.md) for measurements.
