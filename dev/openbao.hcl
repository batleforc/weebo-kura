# The local OpenBao (task dev:vault): a real server, not dev mode, so the
# Transit keys that wrap the vaults' data keys survive a restart. Its storage
# is the podman volume kura-dev-vault-data; the unseal key and root token are
# in dev/openbao-keys.json (gitignored). Loopback only, no TLS: dev use alone.
# OpenBao 2 has no "file" storage: one Raft node.
storage "raft" {
  path    = "/openbao/file"
  node_id = "kura-dev"
}

listener "tcp" {
  address     = "0.0.0.0:8200"
  tls_disable = true
}

api_addr      = "http://127.0.0.1:8200"
cluster_addr  = "http://127.0.0.1:8201"
disable_mlock = true
