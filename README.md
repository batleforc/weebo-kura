# weebo-kura

A self-hosted, multi-tenant sync backend for [Obsidian](https://obsidian.md)
vaults. Devices sign in through OIDC (Authentik). Notes are stored as encrypted,
deduplicated chunks on S3-compatible storage, with keys wrapped by
Vault/OpenBao Transit. The metadata lives in PostgreSQL.

| Part | Where |
| --- | --- |
| Server (axum, PostgreSQL, S3) | [`crates/kura-server`](crates/kura-server) |
| Sync engine, shared by every client | [`crates/kura-core`](crates/kura-core) |
| CLI `kura` | [`crates/kura-cli`](crates/kura-cli) |
| Obsidian plugin (engine compiled to WASM) | [`plugin`](plugin), [`crates/kura-wasm`](crates/kura-wasm) |
| Web console | [`ui`](ui) |
| Helm chart | [`deploy/helm/kura`](deploy/helm/kura) |

## Getting started

Tools come from [mise](https://mise.jdx.dev) and every command is a
[Task](https://taskfile.dev) task (`task --list`).

```sh
mise install
task dev:stack   # PostgreSQL, OpenBao, S3 and Authentik, one container at a time
task dev:run     # kura-server on http://127.0.0.1:58080
task test        # export the KURA_TEST_* variables first (see task dev:up), or the database tests skip
task lint
task audit
```

## Documentation

- [Operating kura-server](docs/operator.md)
- [API reference](docs/api/openapi.json), served by the server under `/scalar`
- [RFCs](docs/readme.md#rfcs)
- [Security policy](SECURITY.md)

## License

[Apache-2.0](LICENSE.md)
