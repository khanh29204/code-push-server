FROM rust:1.90-alpine AS base

RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static
RUN rustup target add x86_64-unknown-linux-musl

FROM base AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target x86_64-unknown-linux-musl

FROM scratch AS runner

WORKDIR /app

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/rust ./code-push-server
COPY public /app/public

ENV PORT=3000

EXPOSE 3000

ENTRYPOINT ["./code-push-server"]