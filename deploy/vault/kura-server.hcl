# The Vault policy of kura-server (RFC 0001 §6.6, decision 33, T18).
#
# Read its two KV v2 secrets; with each tenant's Transit key, only create a
# data key, decrypt one, and rewrap. Nothing deletes or reconfigures a key:
# a compromised server cannot destroy a tenant's data, and the key, hence any
# backup, outlives a `Delete` purge until an operator destroys it under a
# separate role. Mount names are server.yaml's defaults (`mv`, `transit`):
# change them here if yours differ.

path "mv/data/kura/database" {
  capabilities = ["read"]
}

path "mv/data/kura/bucket" {
  capabilities = ["read"]
}

path "transit/datakey/plaintext/kura-*" {
  capabilities = ["update"]
}

path "transit/decrypt/kura-*" {
  capabilities = ["update"]
}

path "transit/rewrap/kura-*" {
  capabilities = ["update"]
}
