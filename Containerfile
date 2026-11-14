# From the weebo-containerfile template (batlehub@20879e5), one server image, no UI.
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

# ── Runtime image ─────────────────────────────────────────────────────────────
# The process writes nothing to disk, so the chart runs it with a read-only root
# filesystem and no writable directory is pre-created.
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:cb87acc574abe719b6e5c547384defa2f7b1f24af9605d3f618cf852495d8be2 AS runtime

COPY --from=builder /build/target/release/kura-server /usr/local/bin/kura-server

# API, then metrics (never exposed by the ingress, RFC 0001 §4.4).
EXPOSE 8080 9090

# Numeric non-root UID, the one the Helm chart's runAsUser pins.
USER 65532:65532

ENTRYPOINT ["kura-server"]
CMD ["--config", "/etc/kura/server.yaml"]
