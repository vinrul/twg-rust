# Hasil stress test web — Gate G1

Renderer: ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 Ti (0x00002182) Direct3D11 vs_5_0 ps_5_0, D3D11) (WebGL 2.0 (OpenGL ES 3.0 Chromium))

Warmup 3s, sample 10s, zoom 19, layer osm, LOD 1, Draco 1.

| Mode | Count | FPS | Worst frame (ms) | Status |
|---|---:|---:|---:|---|
| Animated | 1000 | 99.74 | 23.60 | PASS |
| Animated | 5000 | 54.08 | 88.30 | PASS |
| Animated | 10000 | 26.93 | 147.60 | PASS |
| Static | 1000 | 99.99 | 14.50 | PASS |
| Static | 5000 | 49.60 | 25.10 | PASS |
| Static | 10000 | 27.47 | 43.50 | PASS |
| Mixed 50/50 | 1000 | 99.89 | 19.50 | PASS |
| Mixed 50/50 | 5000 | 51.68 | 57.10 | PASS |
| Mixed 50/50 | 10000 | 26.08 | 112.30 | PASS |

Tile requests: 81; request zooms: 19; Cache Storage entries: 162.
