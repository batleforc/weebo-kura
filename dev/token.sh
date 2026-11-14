#!/usr/bin/env bash
# Prints an access token for a dev user from the local Authentik, through the
# device code flow kura's clients use (§6.2), with no browser: it logs in and
# approves the code through Authentik's flow executor API.
# Each run is a new Authentik session, so a new `sid` (one per device, D34).
# usage: dev/token.sh [user] [password]   (alice, bob or carol / kura-dev)
set -euo pipefail

user=${1:-alice}
password=${2:-kura-dev}
ak=${KURA_DEV_AUTHENTIK:-https://localhost:59443}
here=$(cd "$(dirname "$0")" && pwd)
jar=$(mktemp)
trap 'rm -f "$jar"' EXIT
c() { curl -sfL --cacert "$here/certs/ca.pem" -b "$jar" -c "$jar" "$@"; }

# The flow executor answers each stage, then a POST is CSRF-checked once logged in.
step() { # step <flow> <query> [json]
  local q url csrf
  q=$(printf %s "$2" | jq -sRr @uri)
  url="$ak/api/v3/flows/executor/$1/?query=$q"
  if [ $# -lt 3 ]; then c -H 'content-type: application/json' "$url"; return; fi
  csrf=$(awk '$6=="authentik_csrf"{print $7}' "$jar")
  c -H 'content-type: application/json' -H "Referer: $ak/" -H "X-authentik-CSRF: $csrf" -d "$3" "$url"
}

flow=default-authentication-flow
step $flow "" >/dev/null
step $flow "" "$(jq -nc --arg u "$user" '{component:"ak-stage-identification",uid_field:$u}')" >/dev/null
done=$(step $flow "" "$(jq -nc --arg p "$password" '{component:"ak-stage-password",password:$p}')" | jq -r .component)
[ "$done" = xak-flow-redirect ] || { echo "login refused for $user" >&2; exit 1; }

device=$(c -d client_id=kura-cli -d 'scope=openid profile email offline_access' "$ak/application/o/device/")
code=$(jq -r .user_code <<<"$device")
c -o /dev/null "$ak/device?code=$code"
consent=default-provider-authorization-explicit-consent
s=$(step $consent "code=$code")
if [ "$(jq -r .component <<<"$s")" = ak-stage-consent ]; then
  step $consent "code=$code" "$(jq -c '{component:"ak-stage-consent",token}' <<<"$s")" >/dev/null
fi

c -d client_id=kura-cli -d grant_type=urn:ietf:params:oauth:grant-type:device_code \
  --data-urlencode device_code="$(jq -r .device_code <<<"$device")" "$ak/application/o/token/" | jq -r .access_token
