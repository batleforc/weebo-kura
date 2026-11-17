#!/usr/bin/env bash
# RFC 0003 against the local Authentik (task dev:authentik): the facts its
# sign-in design stands on (decision 13), observed with the
# public client `kura-web` of dev/authentik/blueprints/kura.yaml. A failure
# here after an Authentik upgrade means §6.2 must be read again. The §6.P
# observations (Chromium, the webhook receiver) join this suite in phase 1.
# "note:" lines are behaviours the design tolerates but does not need.
# `console.sh --refresh-token <user>` only signs <user> in as the console and
# prints its refresh token (tests/heavy/reader.sh).
set -euo pipefail
who=alice
[ "${1:-}" = --refresh-token ] && who=${2:?usage: console.sh --refresh-token <user>}

root=$(cd "$(dirname "$0")/../.." && pwd)
ak=${KURA_DEV_AUTHENTIK:-https://localhost:59443}
origin=${KURA_DEV_SERVER:-http://127.0.0.1:58080}
redirect=$origin/app/callback
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
jar=$work/jar
fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok: $*"; }
note() { echo "note: $*"; }
c() { curl -s --cacert "$root/dev/certs/ca.pem" -b "$jar" -c "$jar" "$@"; }
claim() { # claim <jwt> <name>
  jq -Rr --arg n "$2" 'split(".")[1] | gsub("-";"+") | gsub("_";"/") | . + ("=" * ((4 - length % 4) % 4)) | @base64d | fromjson | .[$n] // empty' <<<"$1"
}

# Authentik's flow executor, as dev/approve.sh drives it.
step() { # step <flow> <query> [json]
  local q url csrf
  q=$(printf %s "$2" | jq -sRr @uri)
  url="$ak/api/v3/flows/executor/$1/?query=$q"
  if [ $# -lt 3 ]; then c -L -H 'content-type: application/json' "$url"; return; fi
  csrf=$(awk '$6=="authentik_csrf"{print $7}' "$jar")
  c -L -H 'content-type: application/json' -H "Referer: $ak/" -H "X-authentik-CSRF: $csrf" -d "$3" "$url"
}

# authorize [nopkce]: sets $code and $verifier (not a subshell: both are read).
authorize() {
  local q s challenge consent=default-provider-authorization-explicit-consent
  verifier=$(head -c 48 /dev/urandom | base64 | tr '+/' '-_' | tr -d '=\n')
  challenge=$(printf %s "$verifier" | openssl dgst -sha256 -binary | base64 | tr '+/' '-_' | tr -d '=\n')
  q="client_id=kura-web&response_type=code&scope=openid%20profile%20offline_access&state=s1&redirect_uri=$(jq -rn --arg r "$redirect" '$r|@uri')"
  [ "${1:-}" = nopkce ] || q="$q&code_challenge=$challenge&code_challenge_method=S256"
  c -o /dev/null "$ak/application/o/authorize/?$q"
  s=$(step $consent "$q")
  if [ "$(jq -r .component <<<"$s")" = ak-stage-consent ]; then
    s=$(step $consent "$q" "$(jq -c '{component:"ak-stage-consent",token}' <<<"$s")")
  fi
  code=$(jq -r '.to // empty' <<<"$s" | sed -n 's/.*[?&]code=\([^&]*\).*/\1/p')
}

# token <headers file> <origin> <form fields…>: body, then " http=<status>".
token() {
  local hdr=$1 from=$2; shift 2
  c -D "$hdr" -w ' http=%{http_code}' -H "Origin: $from" "$ak/application/o/token/" -d client_id=kura-web "$@"
}
exchange() { token "$work/h" "$origin" -d grant_type=authorization_code --data-urlencode "code=$code" --data-urlencode "redirect_uri=$redirect" --data-urlencode "code_verifier=$verifier"; }
refresh() { token "$work/h" "${2:-$origin}" -d grant_type=refresh_token --data-urlencode "refresh_token=$1"; }
status() { echo "${1##* http=}"; }
field() { jq -r --arg n "$2" '.[$n] // empty' <<<"${1% http=*}"; }
allow_origin() { tr -d '\r' <"$1" | awk -F': ' 'tolower($1)=="access-control-allow-origin"{print $2}'; }

c -o /dev/null -f "$ak/application/o/kura-web/.well-known/openid-configuration" \
  || fail "no kura-web provider on $ak: task dev:authentik (re-apply the blueprint on an older stack)"
flow=default-authentication-flow
step $flow "" >/dev/null
step $flow "" "{\"component\":\"ak-stage-identification\",\"uid_field\":\"$who\"}" >/dev/null
[ "$(step $flow "" '{"component":"ak-stage-password","password":"kura-dev"}' | jq -r .component)" = xak-flow-redirect ] || fail "$who cannot log in"
if [ "${1:-}" = --refresh-token ]; then
  authorize
  field "$(exchange)" refresh_token
  exit 0
fi

# a. The token endpoint answers the application's origin, and only it.
authorize; [ -n "$code" ] || fail "no authorization code"
t=$(exchange); [ "$(status "$t")" = 200 ] || fail "code exchange: $t"
[ "$(allow_origin "$work/h")" = "$origin" ] || fail "the token answer does not allow $origin"
c -o /dev/null -D "$work/p" -X OPTIONS -H "Origin: $origin" -H 'Access-Control-Request-Method: POST' "$ak/application/o/token/"
[ "$(allow_origin "$work/p")" = "$origin" ] || fail "the preflight does not allow $origin"
c -o /dev/null -D "$work/d" -H "Origin: $origin" "$ak/application/o/kura-web/.well-known/openid-configuration"
[ "$(allow_origin "$work/d")" = "$origin" ] || fail "kura-web's discovery does not allow $origin"
c -o /dev/null -D "$work/d" -H "Origin: $origin" "$ak/application/o/kura/.well-known/openid-configuration"
[ -z "$(allow_origin "$work/d")" ] || note "another application's discovery now allows $origin"
pass "a. a public client exchanges its code from $origin and reads its own discovery (CORS from the redirect URI, per application)"

# b. The access token names its session, on a sign-in and after a refresh.
at=$(field "$t" access_token); rt1=$(field "$t" refresh_token)
sid=$(claim "$at" sid); auth_time=$(claim "$at" auth_time)
[ -n "$sid" ] && [ -n "$auth_time" ] || fail "the code grant's access token lacks sid or auth_time"
t=$(refresh "$rt1" https://evil.example); [ "$(status "$t")" = 200 ] || fail "refresh: $t"
[ -z "$(allow_origin "$work/h")" ] || fail "a foreign origin is allowed to read the token answer"
at=$(field "$t" access_token); rt2=$(field "$t" refresh_token)
[ "$(claim "$at" sid)" = "$sid" ] && [ "$(claim "$at" auth_time)" = "$auth_time" ] || fail "sid or auth_time changed at the refresh"
pass "b. sid and auth_time on the code grant and unchanged after a refresh; a foreign origin gets no CORS header"

# c. Refresh tokens rotate, and a rotated one is refused.
[ -n "$rt2" ] && [ "$rt2" != "$rt1" ] || fail "the refresh token did not rotate (refresh_token_threshold is not 0?)"
t=$(refresh "$rt1")
[ "$(status "$t")" = 400 ] && [ "$(field "$t" error)" = invalid_grant ] || fail "a rotated refresh token is not refused: $t"
pass "c. rotation at each refresh; the rotated token is 400 invalid_grant"
t=$(refresh "$rt2"); rt3=$(field "$t" refresh_token)
if [ -n "$rt3" ]; then
  note "the newer token survives the reuse of the older one: no family revocation (T14)"
else
  note "the reuse revoked the whole family: T14's residual can shrink"
  authorize; rt3=$(field "$(exchange)" refresh_token)
fi

# f. A second sign-in in the same Authentik session is the same session.
authorize; t=$(exchange); at=$(field "$t" access_token); rt_second=$(field "$t" refresh_token)
[ "$(claim "$at" sid)" = "$sid" ] && [ "$(claim "$at" auth_time)" = "$auth_time" ] || fail "a second sign-in changed sid or auth_time (decision 24)"
pass "f. a second sign-in in one Authentik session carries the same sid and auth_time"

# d. A public client revokes its refresh token, without reading the answer.
r=$(c -D "$work/r" -w ' http=%{http_code}' -H "Origin: $origin" "$ak/application/o/revoke/" -d client_id=kura-web --data-urlencode "token=$rt3")
[ "$(status "$r")" = 200 ] || fail "revocation by a public client: $r"
[ "$(status "$(refresh "$rt3")")" = 400 ] || fail "a revoked refresh token still refreshes"
pass "d. a public client revokes its refresh token (client_id and token only)"
if [ -z "$(allow_origin "$work/r")" ]; then
  note "the revocation answer has no CORS header: the browser cannot read it (decision 25)"
else
  note "the revocation answer now allows the origin: sign-out could read it (decision 25)"
fi

# g. Authentik does not require PKCE of a public client (T9's residual).
authorize nopkce
if [ -n "$code" ] && [ "$(status "$(token "$work/h" "$origin" -d grant_type=authorization_code --data-urlencode "code=$code" --data-urlencode "redirect_uri=$redirect")")" = 200 ]; then
  note "a code asked without a challenge is exchanged without a verifier: PKCE is the client's doing (decision 27)"
else
  note "Authentik now refuses a code flow without PKCE: T9's residual can go (decision 27)"
fi

# h. Last, because it logs out: the Authentik session ends, the refresh goes on.
step default-invalidation-flow "" >/dev/null
t=$(refresh "$rt_second"); [ "$(status "$t")" = 200 ] || fail "no refresh once the Authentik session has ended: $t"
at=$(field "$t" access_token)
[ "$(claim "$at" auth_time)" = "$auth_time" ] || fail "auth_time changed once the Authentik session ended"
pass "h. after the Authentik session ends the refresh still works, with the same auth_time"
[ -n "$(claim "$at" sid)" ] || note "that token has no sid: require_session then rests on auth_time [R12]"
echo "console.sh: the Authentik facts of RFC 0003 hold on $ak"
