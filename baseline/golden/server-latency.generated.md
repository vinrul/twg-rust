# Baseline Latency Server Lokal

Diukur pada 2026-11-04T15:26:09.1688768+08:00 terhadap http://127.0.0.1:8010.
Bukan benchmark produksi; loopback, satu proses, fixture lokal.

| Endpoint | Samples | Avg ms | Min ms | P50 ms | P95 ms | Max ms |
|---|---:|---:|---:|---:|---:|---:|
| healthz | 30 | 4.91 | 1.28 | 1.53 | 20.66 | 21.1 |
| readyz | 30 | 6.68 | 1.68 | 2.03 | 26.02 | 26.39 |
| public-operasis | 30 | 6.31 | 2.27 | 2.75 | 24.26 | 26.36 |
| public-markers | 30 | 6.51 | 3.1 | 4.05 | 27.82 | 28.2 |

