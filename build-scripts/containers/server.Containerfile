FROM rust:1.97.1-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends build-essential pkg-config libssl-dev ca-certificates git && rm -rf /var/lib/apt/lists/*
ENV PATH=/usr/local/cargo/bin:$PATH
