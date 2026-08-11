FROM lukemathwalker/cargo-chef:latest-rust-alpine AS chef
RUN apk add --no-cache musl-dev pkgconfig gcc g++

WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release && cp /app/target/release/rust /app/code-push-server

FROM alpine:3.20 AS runner
WORKDIR /app
RUN apk add --no-cache ca-certificates

COPY --from=builder /app/code-push-server /app/code-push-server
COPY public /app/public

ENV PORT=3000
EXPOSE 3000

ENTRYPOINT ["/app/code-push-server"]
