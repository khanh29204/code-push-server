FROM rust:1.90-alpine AS base

RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static
RUN rustup target add x86_64-unknown-linux-musl

FROM base AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

# Build dependencies only (cache layer)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    mkdir src && echo "fn main() {}" > src/main.rs \
    && RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target x86_64-unknown-linux-musl \
    && rm -rf src

# Copy real source và build
COPY src ./src

# Touch main.rs và copy binary từ target cache ra
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    touch src/main.rs \
    && RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target x86_64-unknown-linux-musl \
    && cp /app/target/x86_64-unknown-linux-musl/release/rust /app/code-push-server

FROM scratch AS runner

WORKDIR /app

COPY --from=builder /app/code-push-server ./code-push-server
COPY public /app/public

ENV PORT=3000

EXPOSE 3000

ENTRYPOINT ["./code-push-server"]