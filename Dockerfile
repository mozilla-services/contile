# Docker 17.05 or higher required for multi-stage builds

# Change this to be your application's name
ARG APPNAME=contile

# make sure that the build and run environments are the same version
FROM rust:1.54-slim-buster as builder
ARG APPNAME
ADD . /app
WORKDIR /app

# Make sure that this matches in .travis.yml
# ARG RUST_TOOLCHAIN=nightly
RUN \
    apt-get -qq update && \
    apt-get install libssl-dev pkg-config -y && \
    \
    rustup default ${RUST_TOOLCHAIN} && \
    cargo --version && \
    rustc --version && \
    mkdir -m 755 bin && \
    cargo build --release && \
    cp /app/target/release/${APPNAME} /app/bin


FROM debian:buster-slim
ARG APPNAME

# FROM debian:buster  # for debugging docker build
RUN \
    groupadd --gid 10001 app && \
    useradd --uid 10001 --gid 10001 --home /app --create-home app && \
    \
    apt-get -qq update && \
    apt-get -qq install -y libssl-dev pkg-config ca-certificates && \
    rm -rf /var/lib/apt/lists

COPY --from=builder /app/bin /app/bin
COPY --from=builder /app/version.json /app
COPY --from=builder /app/entrypoint.sh /app

WORKDIR /app
USER app

# ARG variables aren't available at runtime
ENV BINARY=/app/bin/${APPNAME}
ENTRYPOINT ["/app/entrypoint.sh"]
