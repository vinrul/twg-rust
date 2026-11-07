#!/bin/bash
# Smoke test for online-user listing and admin session revocation.
#
# Requires a running server and the baseline fixture users.
#
# Usage:
#   BASE=http://127.0.0.1:8010 \
#   ADMIN_DEVICE=device-1 USER_DEVICE=kick-smoke \
#   bash tools/kick-smoke.sh
set -euo pipefail

BASE=${BASE:-http://127.0.0.1:8010}
ADMIN_USERNAME=${ADMIN_USERNAME:-baselineadmin}
ADMIN_PASSWORD=${ADMIN_PASSWORD:-Baseline#2026}
USER_USERNAME=${USER_USERNAME:-baselineuser}
USER_PASSWORD=${USER_PASSWORD:-Baseline#2026}
ADMIN_DEVICE=${ADMIN_DEVICE:-online-admin-smoke}
USER_DEVICE=${USER_DEVICE:-kick-smoke-device}

login() {
  curl -s -X POST "$BASE/auth/login" -H 'content-type: application/json' \
    -d "{\"username\":\"$1\",\"password\":\"$2\",\"device_id\":\"$3\"}"
}

# Dedicated route for twg-online-admin: admin-only, never single-device.
login_admin_tool() {
  curl -s -X POST "$BASE/auth/login/admin" -H 'content-type: application/json' \
    -d "{\"username\":\"$1\",\"password\":\"$2\",\"device_id\":\"$3\"}"
}

echo '- login user to be kicked'
user=$(login "$USER_USERNAME" "$USER_PASSWORD" "$USER_DEVICE")
uid=$(echo "$user" | jq -r '.data.user.id // empty')
utoken=$(echo "$user" | jq -r '.data.access_token // empty')
if [ -z "$uid" ] || [ -z "$utoken" ]; then
  echo "USER LOGIN FAILED: $user" >&2
  exit 1
fi
echo "$user" | jq -c '{user: .data.user.username, id: .data.user.id}'

echo '- login admin via the admin-tool route (no single-device check)'
admin=$(login_admin_tool "$ADMIN_USERNAME" "$ADMIN_PASSWORD" "$ADMIN_DEVICE")
atoken=$(echo "$admin" | jq -r '.data.access_token // empty')
if [ -z "$atoken" ]; then
  echo "ADMIN LOGIN FAILED: $admin" >&2
  exit 1
fi

echo '- online list (private) contains the user'
online=$(curl -s "$BASE/private/users/online" \
  -H "authorization: Bearer $atoken" -H "x-device-id: $ADMIN_DEVICE")
echo "$online" | jq -c --arg id "$uid" \
  '.data[] | select(.id == $id) | {username, active_session_count, devices: [.sessions[].device_id]}'
echo "$online" | jq -e --arg id "$uid" \
  '.data | any(.id == $id and .active_session_count >= 1)' >/dev/null

echo '- non-admin cannot use the admin-tool login (403)'
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/auth/login/admin" \
  -H 'content-type: application/json' \
  -d "{\"username\":\"$USER_USERNAME\",\"password\":\"$USER_PASSWORD\",\"device_id\":\"tool-non-admin\"}")
test "$code" = "403"

echo '- non-admin cannot kick (403)'
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/admin/users/$uid/kick-session" \
  -H "authorization: Bearer $utoken" -H "x-device-id: $USER_DEVICE")
test "$code" = "403"

echo '- legacy public kick is gone (404)'
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/public/users/$uid/kick-session")
test "$code" = "404"

echo '- admin kick revokes the session'
kicked=$(curl -s -X POST "$BASE/admin/users/$uid/kick-session" \
  -H "authorization: Bearer $atoken" -H "x-device-id: $ADMIN_DEVICE")
echo "$kicked" | jq -c '{username: .data.username, revoked: .data.revoked_sessions}'
echo "$kicked" | jq -e '.data.revoked_sessions >= 1' >/dev/null

echo '- kicked user can log in again'
again=$(login "$USER_USERNAME" "$USER_PASSWORD" "$USER_DEVICE")
echo "$again" | jq -c '{code, message}'
echo "$again" | jq -e '.code == 200' >/dev/null

echo '- cleanup: revoke the new session'
curl -s -X POST "$BASE/admin/users/$uid/kick-session" \
  -H "authorization: Bearer $atoken" -H "x-device-id: $ADMIN_DEVICE" \
  | jq -e '.data.revoked_sessions >= 1' >/dev/null

echo 'KICK SMOKE OK'
