#!/bin/bash
# Smoke test for the asset library: static/animated model uploads, Draco/LOD2
# conversion per source slot, reuse detection, image resize, manifest, cleanup.
#
# Requires: a running server, curl, jq, python3, and `gltf-transform` on PATH
# (only needed when a conversion actually runs).
#
# Note: `admin` is a single-device role (SINGLE_DEVICE_ROLES), so either log out
# the other session or reuse its DEVICE_ID.
#
# Usage:
#   BASE=http://127.0.0.1:8010 DEVICE_ID=device-1 bash tools/assets-smoke.sh
set -euo pipefail

BASE=${BASE:-http://127.0.0.1:8099}
USERNAME=${USERNAME:-baselineadmin}
PASSWORD=${PASSWORD:-Baseline#2026}
DEVICE_ID=${DEVICE_ID:-asset-smoke}
MODEL_STATIC=${MODEL_STATIC:-assets/models/polwan_airud_idle_non_draco.glb}
MODEL_IDLE=${MODEL_IDLE:-assets/models/polwan_airud_idle_non_draco.glb}
MODEL_ANIMATED=${MODEL_ANIMATED:-assets/models/polwan_airud_walk_non_draco.glb}
MODEL_IDLE_LOD2_DRACO=${MODEL_IDLE_LOD2_DRACO:-assets/models/polwan_airud_idle_lod2_draco.glb}
MODEL_ANIMATED_LOD2_DRACO=${MODEL_ANIMATED_LOD2_DRACO:-assets/models/polwan_airud_walk_lod2_draco.glb}

WORKDIR=$(mktemp -d)
CREATED=()

login=$(curl -s -X POST "$BASE/auth/login" -H 'content-type: application/json' \
  -d "{\"username\":\"$USERNAME\",\"password\":\"$PASSWORD\",\"device_id\":\"$DEVICE_ID\"}")
token=$(echo "$login" | jq -r '.data.access_token // empty')
if [ -z "$token" ]; then
  echo "LOGIN FAILED: $login" >&2
  exit 1
fi
AUTH=(-H "authorization: Bearer $token" -H "x-device-id: $DEVICE_ID")

python3 - "$WORKDIR/icon.png" <<'PY'
import struct, sys, zlib

width, height = 320, 160
rows = b"".join(
    bytes([0]) + b"".join(bytes(((x * 3) % 256, (y * 5) % 256, 96, 255)) for x in range(width))
    for y in range(height)
)

def chunk(kind, data):
    body = kind + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

png = (
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(rows, 6))
    + chunk(b"IEND", b"")
)
open(sys.argv[1], "wb").write(png)
PY

cleanup() {
  for asset in "${CREATED[@]}"; do
    curl -s -o /dev/null -X DELETE "$BASE/admin/assets/$asset" "${AUTH[@]}" || true
  done
  rm -rf "$WORKDIR"
}
trap cleanup EXIT

convert() { # id json
  curl -s -X POST "$BASE/admin/assets/$1/convert" "${AUTH[@]}" \
    -H 'content-type: application/json' -d "$2"
}

echo '- reject model upload without model_kind'
code=$(curl -s -o "$WORKDIR/reject.json" -w '%{http_code}' -X POST "$BASE/admin/assets" "${AUTH[@]}" \
  -F "name=No kind" -F "category=model" -F "file=@$MODEL_STATIC")
test "$code" = "400"

echo '- reject animated upload with only one file'
code=$(curl -s -o "$WORKDIR/reject.json" -w '%{http_code}' -X POST "$BASE/admin/assets" "${AUTH[@]}" \
  -F "name=Half animated" -F "category=model" -F "model_kind=animated" \
  -F "idle=@$MODEL_IDLE")
test "$code" = "400"

echo '- reject animated upload whose animated file has no animation'
code=$(curl -s -o "$WORKDIR/reject.json" -w '%{http_code}' -X POST "$BASE/admin/assets" "${AUTH[@]}" \
  -F "name=No animation" -F "category=model" -F "model_kind=animated" \
  -F "idle=@$MODEL_IDLE" -F "animated=@$MODEL_IDLE")
test "$code" = "400"

echo '- reject static model that also sends an animated file'
code=$(curl -s -o "$WORKDIR/reject.json" -w '%{http_code}' -X POST "$BASE/admin/assets" "${AUTH[@]}" \
  -F "name=Static with extra" -F "category=model" -F "model_kind=static" \
  -F "file=@$MODEL_STATIC" -F "animated=@$MODEL_ANIMATED")
test "$code" = "400"

echo '- upload static model'
static=$(curl -s -X POST "$BASE/admin/assets" "${AUTH[@]}" \
  -F "name=Smoke static" -F "category=model" -F "model_kind=static" \
  -F "file=@$MODEL_STATIC")
id=$(echo "$static" | jq -r '.data.id')
CREATED+=("$id")
echo "$static" | jq -e '.data.model_kind == "static" and (.data.sources | length) == 1' >/dev/null
echo "$static" | jq -e '.data.sources[0].slot == "static" and .data.sources[0].has_draco == false' >/dev/null

echo '- convert static draco / lod2 / draco_lod2'
convert "$id" '{"target":"draco"}' | jq -e '.data.converted == true and .data.slot == "static"' >/dev/null
convert "$id" '{"target":"lod2"}' | jq -e '.data.converted == true' >/dev/null
convert "$id" '{"target":"draco"}' | jq -e '.data.converted == false' >/dev/null
convert "$id" '{"target":"draco_lod2"}' | jq -e '.data.converted == true' >/dev/null
echo "- download /assets/$id/static/draco.glb"
code=$(curl -s -o "$WORKDIR/draco.glb" -w '%{http_code}' "$BASE/assets/$id/static/draco.glb")
test "$code" = "200"

echo '- upload animated model (idle + animated)'
animated=$(curl -s -X POST "$BASE/admin/assets" "${AUTH[@]}" \
  -F "name=Smoke animated" -F "category=model" -F "model_kind=animated" \
  -F "idle=@$MODEL_IDLE" -F "animated=@$MODEL_ANIMATED")
id_animated=$(echo "$animated" | jq -r '.data.id')
CREATED+=("$id_animated")
echo "$animated" | jq -e '.data.model_kind == "animated"' >/dev/null
echo "$animated" | jq -e '[.data.sources[].slot] == ["idle", "animated"]' >/dev/null

echo '- convert draco per slot'
convert "$id_animated" '{"target":"draco","slot":"idle"}' \
  | jq -e '.data.converted == true and .data.slot == "idle"' >/dev/null
convert "$id_animated" '{"target":"draco_lod2","slot":"animated"}' \
  | jq -e '.data.converted == true and .data.slot == "animated"' >/dev/null
echo "- idle slot keeps animated slot untouched"
curl -s "$BASE/private/assets/$id_animated" "${AUTH[@]}" \
  | jq -e '[.data.sources[] | select(.slot == "idle") | .variants[].kind] == ["draco"]' >/dev/null
curl -s "$BASE/private/assets/$id_animated" "${AUTH[@]}" \
  | jq -e '[.data.sources[] | select(.slot == "animated") | .variants[].kind] == ["draco_lod2"]' >/dev/null

echo '- animated upload already draco+lod2 on both slots is reused'
already=$(curl -s -X POST "$BASE/admin/assets" "${AUTH[@]}" \
  -F "name=Smoke animated draco lod2" -F "category=model" -F "model_kind=animated" \
  -F "idle=@$MODEL_IDLE_LOD2_DRACO" -F "animated=@$MODEL_ANIMATED_LOD2_DRACO")
id_already=$(echo "$already" | jq -r '.data.id')
CREATED+=("$id_already")
echo "$already" | jq -e '[.data.sources[] | {draco: .has_draco, lod}] | all(.draco == true and .lod == 2)' >/dev/null
convert "$id_already" '{"target":"draco_lod2","slot":"idle"}' \
  | jq -e '.data.converted == false and .data.asset.sources[0].variants[0].reused == true' >/dev/null
convert "$id_already" '{"target":"draco_lod2","slot":"animated"}' \
  | jq -e '.data.converted == false' >/dev/null

echo '- animated convert without slot is rejected'
code=$(curl -s -o "$WORKDIR/reject.json" -w '%{http_code}' -X POST \
  "$BASE/admin/assets/$id_animated/convert" "${AUTH[@]}" \
  -H 'content-type: application/json' -d '{"target":"draco"}')
test "$code" = "400"

echo '- icon resize to 128'
icon=$(curl -s -X POST "$BASE/admin/assets" "${AUTH[@]}" \
  -F "name=Smoke icon" -F "category=icon" -F "file=@$WORKDIR/icon.png")
id_icon=$(echo "$icon" | jq -r '.data.id')
CREATED+=("$id_icon")
echo "$icon" | jq -e '.data.model_kind == null and .data.sources[0].slot == "static"' >/dev/null
resized=$(convert "$id_icon" '{"target":"resize","size":128}')
echo "$resized" | jq -e '.data.converted == true' >/dev/null
echo "$resized" | jq -e '.data.asset.sources[0].variants[0].width == 128' >/dev/null

echo '- manifest lists the created assets'
manifest=$(curl -s "$BASE/private/assets/manifest" "${AUTH[@]}")
echo "$manifest" | jq -e --argjson count "${#CREATED[@]}" '.data.assets | length >= $count' >/dev/null

echo 'ASSET SMOKE OK'
