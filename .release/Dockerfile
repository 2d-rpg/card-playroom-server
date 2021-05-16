FROM rust:1.47.0-slim

RUN apt update \
    && apt install -y --no-install-recommends \
    apt-utils \
    git \
    ca-certificates \
    libpq-dev \
    postgresql \
    && apt autoremove -y \
    && apt clean -y \
    && rm -rf /var/lib/apt/lists/* \
    && cargo install diesel_cli --no-default-features --features postgres \
    && diesel migration run \
    && cargo run
