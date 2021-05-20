# Docker 17.05 or higher required for multi-stage builds

# Change this to be your application's name
ARG APPNAME=contile

FROM rust:1.52.1 as builder
ARG APPNAME
ADD . /app
WORKDIR /app

# Make sure that this matches in .travis.yml
# ARG RUST_TOOLCHAIN=nightly
RUN \
    apt-get -qq update && \
    \
    rustup default ${RUST_TOOLCHAIN} && \
    cargo --version && \
    rustc --version && \
    mkdir -m 755 bin && \
    cargo build --release && \
    cp /app/target/release/${APPNAME} /app/bin


FROM debian:stretch-slim
ARG APPNAME

# FROM debian:stretch  # for debugging docker build
RUN \
    groupadd --gid 10001 app && \
    useradd --uid 10001 --gid 10001 --home /app --create-home app && \
    \
    apt-get -qq update && \
    apt-get -qq install -y libssl-dev ca-certificates && \
    rm -rf /var/lib/apt/lists

COPY --from=builder /app/bin /app/bin
COPY --from=builder /app/version.json /app
COPY --from=builder /app/entrypoint.sh /app

WORKDIR /app
USER app

# ARG variables aren't available at runtime
ENV BINARY=/app/bin/${APPNAME}
ENTRYPOINT ["/app/entrypoint.sh"]
