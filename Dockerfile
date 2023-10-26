# The Fleek Network Lightning Docker container is hosted at:
# https://github.com/fleek-network/lightning/pkgs/container/lightning
FROM rust:latest as build
WORKDIR /build

RUN apt-get update
RUN apt-get install -y \
    build-essential \
    cmake \
    clang \
    pkg-config \
    libssl-dev \
    gcc \
    protobuf-compiler

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install cargo-strip

COPY . .
ENV RUST_BACKTRACE=1

RUN mkdir -p /build/target/release
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --profile release --bin lightning-node && \
    cargo strip && \
    cp /build/target/release/lightning-node /build

FROM ubuntu:latest
ARG LIGHTNING_PORTS="4200-4299 4300-4399"
ARG USERNAME="lgtn"
WORKDIR /home/$USERNAME
SHELL ["/bin/bash", "-c"] 

RUN apt-get update && \
    apt-get install -y \
    libssl-dev \
    ca-certificates \
    curl

COPY --from=build /build/lightning-node /usr/local/bin/lgtn

RUN useradd -Um $USERNAME

COPY <<EOF /home/$USERNAME/init
#!/usr/bin/bash

if [[ ! -d /home/$USERNAME/.lightning/keystore ]]; then
  lgtn keys generate
fi

lgtn -c /home/$USERNAME/.lightning/config.toml -vv run
EOF

RUN chown $USERNAME:$USERNAME /home/$USERNAME/init
RUN chmod +x /home/$USERNAME/init

EXPOSE $LIGHTNING_PORTS

USER $USERNAME

ENTRYPOINT ["/home/lgtn/init"]