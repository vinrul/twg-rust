# Tools

## `web-stress.mjs`

Automated Gate G1 browser stress run. It serves `apps/web/dist`, launches
Chrome/Edge with the real GPU, drives the page through the DevTools Protocol,
and reads the result the client publishes to `document.title`. No npm
dependencies are required (Node 22+ has a global `WebSocket`).

```bash
cd apps/web
trunk build --cargo-profile wasm-release
cd ../..
node tools/web-stress.mjs --sample=10
```

Options:

| Flag | Default | Meaning |
|---|---|---|
| `--sample=<s>` | `10` | Benchmark sampling duration |
| `--warmup=<s>` | `3` | Warmup before sampling |
| `--zoom=<z>` | `19` | Camera zoom; values above provider max use overzoom |
| `--layer=<osm\|satellite>` | `osm` | Base layer |
| `--lod=<1\|0>` | `1` | Use the 2.4k-triangle marker proxy |
| `--only=<mode:count>` | all | Run one scenario, e.g. `animated:10000` |
| `--switch-layer` | off | Press `L` after the last run and measure the layer switch |
| `--wheel-test` | off | Verify one wheel notch does not jump to min/max zoom |
| `--pan-test` | off | Drag at bearing 45° and verify forward/backward movement |
| `--lod-toggle-test` | off | Press `M` and verify full → LOD reload |
| `--select-test` | off | Click the center marker and verify selection |
| `--memory-test` | off | Run load/unload memory probe and read MB values |
| `--proxy=<url>` | — | Tile proxy base URL passed as `?proxy=` |
| `--proxy-test` | off | Verify tiles load through the tile proxy (same-origin harness route by default) |
| `--fly-test` | off | Double-click fly-to and compass reset |
| `--tag=<name>` | — | Suffix for report and screenshot filenames |
| `--headless=false` | headless | Show the browser window |
| `--timeout=<ms>` | `240000` | Per-scenario timeout |

Reports are written to `docs/web-stress-results*.json`, `.md`, and
`docs/web-stress-console*.log`. Screenshots go to
`docs/assets/web-stress/`.

`--proxy-test` proxies `/tiles/*` through the harness origin to
`TILE_PROXY_BACKEND` (default `http://127.0.0.1:8010`) and then verifies the
service-worker Cache Storage entries. This keeps the test same-origin, which is
what the browser client sees in production. Pass `--proxy=<url>` to test a
different proxy origin directly.

Set `CHROME_PATH` if Chrome/Edge is not in a standard location.

## `phase4-smoke.mjs`

Auth smoke test against a running server and PostgreSQL. It checks login,
single-device conflict, identify, refresh rotation/replay, logout, revoked
session, and wrong password.

```bash
BASE_URL=http://127.0.0.1:8011 \
TWG_USERNAME=baselineadmin TWG_PASSWORD='Baseline#2026' \
node tools/phase4-smoke.mjs
```

Use `TWG_USERNAME` (not `USERNAME`) because Windows sets `USERNAME` to the OS
account name.

## `paseto-parity.mjs`

Bidirectional auth parity check. Both servers must share the same baseline
database and `TOKEN_SYMMETRIC_KEY`. It verifies that the Rust server accepts a
legacy Go PASETO/refresh token and that the legacy server accepts Rust tokens.

```bash
OLD_URL=http://127.0.0.1:8013 NEW_URL=http://127.0.0.1:8011 \
node tools/paseto-parity.mjs
```

Use `TWG_USERNAME` to test a multi-device role such as `baselineuser`; the
default `baselineadmin` exercises the single-device path.

## `read-parity.mjs`

Differential check for every read-only endpoint. Both servers must share the
same baseline database and `TOKEN_SYMMETRIC_KEY`. The script compares status,
content type, `X-Request-ID`, parsed JSON body, list order, `total`, null/omit,
and timestamps across all roles and pagination edge cases.

```bash
OLD_URL=http://127.0.0.1:8013 NEW_URL=http://127.0.0.1:8011 \
TWG_PASSWORD='Baseline#2026' node tools/read-parity.mjs
```

| Env | Default | Meaning |
|---|---|---|
| `OLD_URL` | `http://127.0.0.1:8013` | Legacy Go server |
| `NEW_URL` | `http://127.0.0.1:8011` | Rust server |
| `TWG_PASSWORD` | `Baseline#2026` | Password for all baseline accounts |

Exit code is non-zero on any mismatch. Three 403 checks are skipped because the
legacy Windows binary panics in zerolog; their golden fixtures are checked
instead. The same harness can be pointed at shadow traffic by setting `OLD_URL`
to the production origin.
