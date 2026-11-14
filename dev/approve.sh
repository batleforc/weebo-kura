#!/usr/bin/env bash
# Approves a device code in the local Authentik as a dev user, with no
# browser: logs in and consents through Authentik's flow executor API.
# usage: dev/approve.sh <user code> [user] [password]   (alice, bob or carol / kura-dev)
set -euo pipefail

code=$1
user=${2:-alice}
password=${3:-kura-dev}
ak=${KURA_DEV_AUTHENTIK:-https://localhost:59443}
here=$(cd "$(dirname "$0")" && pwd)
jar=$(mktemp)
trap 'rm -f "$jar"' EXIT
c() { curl -sfL --cacert "$here/certs/ca.pem" -b "$jar" -c "$jar" "$@"; }

# The flow executor answers each stage; once logged in, a POST is CSRF-checked.
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

c -o /dev/null "$ak/device?code=$code"
consent=default-provider-authorization-explicit-consent
s=$(step $consent "code=$code")
if [ "$(jq -r .component <<<"$s")" = ak-stage-consent ]; then
  s=$(step $consent "code=$code" "$(jq -c '{component:"ak-stage-consent",token}' <<<"$s")")
fi
[ "$(jq -r .component <<<"$s")" = ak-provider-oauth2-device-code-finish ] || { echo "code $code not approved" >&2; exit 1; }
