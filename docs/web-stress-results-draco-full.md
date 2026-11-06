# Hasil stress test web — Gate G1

Renderer: ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 Ti (0x00002182) Direct3D11 vs_5_0 ps_5_0, D3D11) (WebGL 2.0 (OpenGL ES 3.0 Chromium))

Warmup 3s, sample 10s, zoom 19, layer osm, LOD 0, Draco 1.

| Mode | Count | FPS | Worst frame (ms) | Status |
|---|---:|---:|---:|---|
| Animated | 1000 | 68.35 | 48.70 | PASS |
| Animated | 5000 | 14.26 | 250.00 | PASS |
| Animated | 10000 | 7.23 | 250.00 | PASS |
| Static | 1000 | 65.19 | 19.80 | PASS |
| Static | 5000 | 13.22 | 83.90 | PASS |
| Static | 10000 | 6.82 | 155.70 | PASS |
| Mixed 50/50 | 1000 | 64.97 | 31.90 | PASS |
| Mixed 50/50 | 5000 | 13.64 | 127.60 | PASS |
| Mixed 50/50 | 10000 | 7.43 | 249.40 | PASS |

Tile requests: 81; request zooms: 19; Cache Storage entries: 162.
