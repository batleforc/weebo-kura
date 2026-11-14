#!/usr/bin/env bash
# Prints an access token for a dev user from the local Authentik, through the
# device code flow kura's clients use (§6.2), approved by dev/approve.sh.
# Each run is a new Authentik session, so a new `sid` (one per device, D34).
# usage: dev/token.sh [user] [password]   (alice, bob or carol / kura-dev)
set -euo pipefail

user=${1:-alice}
password=${2:-kura-dev}
ak=${KURA_DEV_AUTHENTIK:-https://localhost:59443}
here=$(cd "$(dirname "$0")" && pwd)
c() { curl -sf --cacert "$here/certs/ca.pem" "$@"; }

# Authentik answers 429 slow_down to device requests in quick succession
# (RFC 8628 §3.5): wait and ask again, for up to two minutes; asking more
# often keeps the throttle on.
device=""
for _ in $(seq 6); do
  device=$(c -d client_id=kura-cli -d 'scope=openid profile email offline_access' "$ak/application/o/device/") && break
  sleep 20
done
[ -n "$device" ] || { echo "token.sh: no device code from $ak" >&2; exit 1; }
"$here/approve.sh" "$(jq -r .user_code <<<"$device")" "$user" "$password"
c -d client_id=kura-cli -d grant_type=urn:ietf:params:oauth:grant-type:device_code \
  --data-urlencode device_code="$(jq -r .device_code <<<"$device")" "$ak/application/o/token/" | jq -r .access_token
