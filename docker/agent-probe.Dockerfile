# S01 EXP-04 agent probe image. Base images are pinned by digest (ADR-013).
# The image carries one Rust binary and the system CA bundle. It must never
# contain a coding CLI, Node, Python, game files, credentials or recordings.
# The OpenAI key is passed at run time only: docker run -e OPENAI_API_KEY ...
FROM rust:1.97.1-slim-bookworm@sha256:2775a09d208ff0d7c1f50490c45b62db929e87ba1dcbc3f2132ac71a704bcdd3 AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY probes/runtime/Cargo.toml probes/runtime/Cargo.toml
COPY probes/runtime/src probes/runtime/src
RUN cargo build --release --locked -p factory-runtime-probe

FROM debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8
# TLS roots for the platform certificate verifier used by reqwest/rustls.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*
RUN useradd --system --uid 10001 --no-create-home factory
COPY --from=build /src/target/release/runtime-probe /usr/local/bin/runtime-probe
USER factory
ENTRYPOINT ["/usr/local/bin/runtime-probe"]
CMD ["live-once"]
