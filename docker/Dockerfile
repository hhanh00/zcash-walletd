#syntax=docker/dockerfile:1.2
FROM rust:1.57 as builder

RUN rustup component add rustfmt
COPY . /zcash-walletd

RUN cd /zcash-walletd && cargo build --release

FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y --no-install-recommends sqlite3 ca-certificates
RUN update-ca-certificates

COPY --from=builder /zcash-walletd/target/release/zcash-walletd ./root
COPY --from=builder /zcash-walletd/Rocket.toml ./root

WORKDIR /root
ENV PATH $PATH:/root
ENV RUST_LOG info
VOLUME /data

ENTRYPOINT ["zcash-walletd"]
