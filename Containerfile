FROM docker.io/library/rust:latest AS builder
WORKDIR /usr/src

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools

RUN cargo new mk-app
WORKDIR /usr/src/mk-app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates

RUN cargo build --verbose --release --target x86_64-unknown-linux-musl

FROM scratch
COPY --from=builder /usr/src/mk-app/target/x86_64-unknown-linux-musl/release/mk .
COPY COPYRIGHT LICENSE-APACHE LICENSE-MIT .
USER 1000
CMD ["./mk"]
