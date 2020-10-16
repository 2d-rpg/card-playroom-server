FROM rust:1.47.0-slim

# Install fundamental tools
RUN apt update \
    && apt install -y --no-install-recommends \
    apt-utils \
    git \
    ca-certificates \
    && apt autoremove -y \
    && apt clean -y \
    && rm -rf /var/lib/apt/lists/* \
