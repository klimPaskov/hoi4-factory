# HOI4 Factory server image. Base images are pinned by digest (ADR-013).
# The image must never contain game files, credentials, user data or a coding CLI.
FROM rust:1.97.1-slim-bookworm@sha256:2775a09d208ff0d7c1f50490c45b62db929e87ba1dcbc3f2132ac71a704bcdd3 AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
# Every workspace member must be present for Cargo to load the workspace, even when
# only the server is built. The probe crate is compiled into neither image stage.
COPY probes/runtime/Cargo.toml ./probes/runtime/Cargo.toml
COPY probes/runtime/src ./probes/runtime/src
RUN cargo build --release --locked -p factory-server

FROM debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8
RUN useradd --system --uid 10001 --no-create-home factory
COPY --from=build /src/target/release/factory-server /usr/local/bin/factory-server
USER factory
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/factory-server"]
