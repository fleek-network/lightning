FROM rust:latest as builder
ARG PROFILE=release
WORKDIR /builder

RUN apt-get update && apt-get install -y \
    clang \
    cmake \
    libclang-dev

COPY . .

ENV RUST_BACKTRACE=1

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/builder/target \
    cargo build --profile $PROFILE --bin global-metrics \
    && rm -rf /builder/global-metrics \
    && mv /builder/target/release/global-metrics /builder

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    openssl \
    ca-certificates \
    && apt-get clean \
    && apt-get purge -y \
    && rm -rf /var/lib/apt/lists*

# Get compiled binaries from builder's cargo install directory
COPY --from=builder /builder/global-metrics /usr/local/bin

ENTRYPOINT ["global-metrics"]