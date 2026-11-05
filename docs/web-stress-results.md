# Hasil stress test web — Gate G1

Renderer: ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 Ti (0x00002182) Direct3D11 vs_5_0 ps_5_0, D3D11) (WebGL 2.0 (OpenGL ES 3.0 Chromium))

Warmup 3s, sample 10s, zoom 19, layer osm, LOD 1.

| Mode | Count | FPS | Worst frame (ms) | Status |
|---|---:|---:|---:|---|
| Animated | 1000 | 99.99 | 19.00 | PASS |
| Animated | 5000 | 88.78 | 55.40 | PASS |
| Animated | 10000 | 50.60 | 107.30 | PASS |
| Static | 1000 | 99.99 | 17.40 | PASS |
| Static | 5000 | 92.10 | 19.70 | PASS |
| Static | 10000 | 47.19 | 25.80 | PASS |
| Mixed 50/50 | 1000 | 98.49 | 166.90 | PASS |
| Mixed 50/50 | 5000 | 94.57 | 41.10 | PASS |
| Mixed 50/50 | 10000 | 48.60 | 64.10 | PASS |

Tile requests: 81; request zooms: 19; Cache Storage entries: 162.
