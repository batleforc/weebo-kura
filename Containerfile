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

# ── Plugin: the WASM core (RFC 0001 §6.8), then the Obsidian plugin around it ─
# The console offers it on its Help page (`/app/kura-sync.zip`): the image
# carries the plugin this server expects. Same tools as `task wasm:build`:
# wasm-bindgen as mise.toml (it must equal the crate's version), binaryen as
# mise.lock, both pinned. shortcut: binaryen's x86_64 build only; an arm64
# image needs the aarch64 tarball and its checksum.
FROM chef AS wasm-builder
RUN rustup target add wasm32-unknown-unknown \
 && cargo install --locked wasm-bindgen-cli@0.2.130
ADD --checksum=sha256:2dc9c7813f5375db93d96ead4b78222fcc3e2677bbb832297af4797782a37489 \
    https://github.com/WebAssembly/binaryen/releases/download/version_133/binaryen-version_133-x86_64-linux.tar.gz /tmp/binaryen.tar.gz
RUN tar -xzf /tmp/binaryen.tar.gz -C /opt && rm /tmp/binaryen.tar.gz
COPY --from=planner /build/recipe.json recipe.json
COPY patches/ patches/
RUN cargo chef cook --locked --profile wasm --target wasm32-unknown-unknown -p kura-wasm --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --locked -p kura-wasm --profile wasm --target wasm32-unknown-unknown \
 && wasm-bindgen --target web --out-dir /build/wasm --out-name kura target/wasm32-unknown-unknown/wasm/kura_wasm.wasm \
 && rm -f /build/wasm/kura_bg.wasm.d.ts \
 && /opt/binaryen-version_133/bin/wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
      --enable-sign-ext --enable-mutable-globals -o /build/wasm/kura_bg.wasm /build/wasm/kura_bg.wasm

FROM docker.io/library/node:24-trixie-slim@sha256:af02d57b57719b9657a45199a79264a9437940cd55b3c7b3fc99615654f124a0 AS plugin-builder
WORKDIR /src/plugin
RUN npm install -g --ignore-scripts pnpm@12.21.0
COPY plugin/package.json plugin/pnpm-lock.yaml plugin/pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile
COPY plugin/ ./
COPY --from=wasm-builder /build/wasm/ src/wasm/
# esbuild, with the 3 MB budget of decision 54.
RUN pnpm run build

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
# The plugin the Help page offers; the build fails without it rather than ship a console with no download.
COPY --from=plugin-builder /src/plugin/main.js /src/plugin/manifest.json /src/plugin/styles.css /src/plugin/
# Generates the HeyApi client, type-checks, builds dist/.
RUN KURA_PLUGIN_REQUIRED=1 pnpm run build

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
