# From the weebo-containerfile template (batlehub@20879e5), one server image, with the web console (RFC 0003).
# Keep Containerfile.hardened in step with this file.
# ── Build stage ───────────────────────────────────────────────────────────────
# Pin every base by tag AND digest; updatecli moves both. Same Rust as mise.toml.
FROM docker.io/library/rust:1.100-slim-trixie@sha256:e9d95b74296f7126a83ae3d041aeaba1ff4c478f13abc5cc034fcf7c131c3d4a AS chef

# cargo-chef builds the dependency layer from a recipe. Pinned like the bases.
RUN cargo install --locked cargo-chef@0.1.78

WORKDIR /build

# ── Planner: workspace manifests → recipe.json ────────────────────────────────
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY patches/ patches/
RUN cargo chef prepare --recipe-path recipe.json

# ── Builder: dependencies (cached while recipe.json is unchanged), then sources ─
FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
# The [patch.crates-io] stub must exist before the dependency layer resolves.
COPY patches/ patches/
# --locked: the image must not resolve a dependency the repository never saw.
RUN cargo chef cook --locked --release -p kura-server --recipe-path recipe.json

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --locked --release -p kura-server

# ── UI: the web console (RFC 0003 §6.1), built from the committed spec ────────
# Not from the built binary, as batlehub does: `task openapi:check` already
# holds docs/api/openapi.json equal to the handlers, and this stage then does
# not wait for the Rust build (the template's TD-0003 §4). Same Node as mise.toml.
FROM docker.io/library/node:24-trixie-slim@sha256:af02d57b57719b9657a45199a79264a9437940cd55b3c7b3fc99615654f124a0 AS ui-builder
WORKDIR /src/ui
# Keep in step with `packageManager` in ui/package.json; no install scripts.
RUN npm install -g --ignore-scripts pnpm@12.21.0
COPY ui/package.json ui/pnpm-lock.yaml ui/pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile
COPY docs/api/openapi.json /src/docs/api/openapi.json
COPY ui/ ./
# Generates the HeyApi client, type-checks, builds dist/.
RUN pnpm run build

# ── Runtime image ─────────────────────────────────────────────────────────────
# The process writes nothing to disk, so the chart runs it with a read-only root
# filesystem and no writable directory is pre-created.
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:cb87acc574abe719b6e5c547384defa2f7b1f24af9605d3f618cf852495d8be2 AS runtime

COPY --from=builder /build/target/release/kura-server /usr/local/bin/kura-server
# The console, served under /app when `ui.enabled` (its default `ui.dir`).
COPY --from=ui-builder /src/ui/dist /usr/share/kura/ui

# API, then metrics (never exposed by the ingress, RFC 0001 §4.4).
EXPOSE 8080 9090

# Numeric non-root UID, the one the Helm chart's runAsUser pins.
USER 65532:65532

ENTRYPOINT ["kura-server"]
CMD ["--config", "/etc/kura/server.yaml"]
