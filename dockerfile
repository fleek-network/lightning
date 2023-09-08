FROM rust:1.72.0-slim-bookworm as builder
RUN apt-get update && \
  apt-get upgrade && \
  apt-get -y install build-essential cmake clang pkg-config libssl-dev gcc protobuf-compiler
WORKDIR /usr/lightning
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y libssl-dev ca-certificates
WORKDIR /usr/lightning
COPY --from=builder /usr/lightning/target/release/lightning-node lgtn
CMD ["./lgtn", "run"]
