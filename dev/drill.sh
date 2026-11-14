#!/usr/bin/env bash
# The restore drill of RFC 0001 §10.3 on the dev stack (task dev:drill):
# back up PostgreSQL then the bucket, lose the objects, restore both into a
# separate database, regenerate the epochs, decrypt a sample of chunks.
# OpenBao's Transit keys stay in memory here: their Raft snapshot is the
# cluster drill's part (docs/operator.md §12).
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
work=$(mktemp -d)
trap 'rm -rf "$work"; podman exec kura-dev-postgres dropdb -U postgres --if-exists kura_drill' EXIT
sql() { podman exec kura-dev-postgres psql -U postgres -d "$1" -tAc "$2"; }
server() { KURA_VAULT_TOKEN=kura-dev SSL_CERT_FILE=dev/certs/ca.pem cargo run -q -p kura-server -- "$@"; }

before=$(sql kura "SELECT count(*) FROM chunk")
[ "$before" -gt 0 ] || { echo "no chunk to back up: sync a vault first"; exit 1; }

# 1. In the cluster, PostgreSQL is recovered to T from its WAL archive. The
# dev stack has none: the dump is taken just before the copy, outside the
# maintenance window and with no `maintain` running, so nothing deletes an
# object it names before the copy lists it.
podman exec kura-dev-postgres pg_dump -U postgres -Fc kura >"$work/kura.dump"
podman exec kura-dev-s3 mkdir -p /data/kura-backup
copied=$(KURA_BACKUP_ENDPOINT=http://127.0.0.1:59000 KURA_BACKUP_BUCKET=kura-backup \
  KURA_BACKUP_ACCESS_KEY_ID=kura-dev KURA_BACKUP_SECRET_ACCESS_KEY=kura-dev-secret \
  server backup --config dev/server.yaml)
echo "backup: $copied"

# 2. The disaster: every vault's objects are gone, the database is replaced.
# Never before every vault holding chunks is in the copy.
vaults=$(sql kura "SELECT DISTINCT vault_id FROM chunk")
for v in $vaults; do
  podman exec kura-dev-s3 test -d "/data/kura-backup/$v" || { echo "FAIL: vault $v is not in the copy; nothing deleted"; exit 1; }
done
for v in $vaults; do podman exec kura-dev-s3 rm -rf "/data/kura-dev/$v"; done

# 3. Restore all of it: the database (into kura_drill), then the objects.
podman exec kura-dev-postgres createdb -U postgres kura_drill
podman exec -i kura-dev-postgres pg_restore -U postgres -d kura_drill --no-owner <"$work/kura.dump"
for v in $vaults; do podman exec kura-dev-s3 cp -r "/data/kura-backup/$v" /data/kura-dev/; done
drill=postgres://postgres:kura-dev@127.0.0.1:55433/kura_drill
old=$(sql kura_drill "SELECT string_agg(epoch::text, ',' ORDER BY id) FROM vault")
KURA_DATABASE_URL=$drill server reset-epochs
new=$(sql kura_drill "SELECT string_agg(epoch::text, ',' ORDER BY id) FROM vault")
[ "$old" != "$new" ] || { echo "FAIL: epochs unchanged"; exit 1; }

# 4. Decrypt a sample with the restored rows, objects and Transit keys.
KURA_DATABASE_URL=$drill server check-chunks --sample 50 --config dev/server.yaml
[ "$(sql kura_drill "SELECT count(*) FROM chunk")" = "$before" ] || { echo "FAIL: chunk rows differ"; exit 1; }
echo "ok: drill restored $before chunk rows and their objects, epochs regenerated, sample decrypted"
