# Docker 17.05 or higher required for multi-stage builds

# Change this to be your application's name
ARG APPNAME=contile
# This build arg is used to pass the version (e.g. the commit SHA1 hash) from CI
# when building the application.
ARG VERSION=unset

# !!!NOTE!!!: Ensure builder's Rust version matches CI's in .circleci/config.yml

FROM lukemathwalker/cargo-chef:latest-rust-1.68-bullseye AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG VERSION
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN CONTILE_VERSION=${VERSION} cargo build --release

FROM debian:bullseye-slim AS runtime
ARG APPNAME
WORKDIR /app

RUN \
    groupadd --gid 10001 app && \
    useradd --uid 10001 --gid 10001 --home /app --create-home app && \
    \
    apt-get -qq update && \
    apt-get -qq install -y libssl-dev pkg-config ca-certificates && \
    rm -rf /var/lib/apt/lists && \
    mkdir -m 755 bin

COPY --from=builder /app/target/release/${APPNAME} /app/bin
COPY --from=builder /app/version.json /app
COPY --from=builder /app/entrypoint.sh /app

# ARG variables aren't available at runtime
ENV BINARY=/app/bin/${APPNAME}
ENTRYPOINT ["/app/entrypoint.sh"]
