ARG RUST_VERSION=1.97
FROM rust:${RUST_VERSION}-bookworm
COPY scripts/install-linux-deps.sh /tmp/install-linux-deps.sh
RUN bash /tmp/install-linux-deps.sh && rm -rf /var/lib/apt/lists/*
ENV CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0
WORKDIR /workspace
