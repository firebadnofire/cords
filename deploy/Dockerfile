# syntax=docker/dockerfile:1.7
FROM rust:1.97.1-bookworm AS builder
WORKDIR /source
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY bins/cords-server ./bins/cords-server
COPY bins/cords-client/src-tauri ./bins/cords-client/src-tauri
COPY migrations ./migrations
RUN cargo build --locked --release --bin cords-server

FROM debian:bookworm-slim AS runtime
ARG VCS_REF=unknown
ARG VERSION=0.1.0
LABEL org.opencontainers.image.title="Cords Server" \
      org.opencontainers.image.source="https://github.com/firebadnofire/cords" \
      org.opencontainers.image.revision="${VCS_REF}" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.licenses="AGPL-3.0-only"
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 4848 cords \
    && useradd --uid 4848 --gid 4848 --no-create-home --home-dir /var/lib/cords cords \
    && install -d -o 4848 -g 4848 /var/lib/cords /opt/cords/migrations/postgres \
    && rm -rf /usr/bin/apt* /usr/bin/dpkg* /usr/lib/apt /var/lib/dpkg
COPY --from=builder /source/target/release/cords-server /usr/local/bin/cords-server
COPY --from=builder /source/migrations/postgres /opt/cords/migrations/postgres
COPY deploy/server.toml /opt/cords/server.toml
USER 4848:4848
VOLUME ["/var/lib/cords"]
EXPOSE 4848
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=5 CMD ["cords-server", "healthcheck"]
ENTRYPOINT ["cords-server"]
CMD ["--config", "/opt/cords/server.toml", "serve", "--migrate"]
