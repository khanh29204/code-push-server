FROM rust:1.90-alpine AS chef
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo install cargo-chef

WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - Layer này được Docker cache 100% khi Cargo.toml/Cargo.lock không thay đổi
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-target,target=/app/target \
    RUSTFLAGS="-C target-feature=+crt-static" cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

# Copy source code thực tế và build ứng dụng
COPY . .
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-target,target=/app/target \
    RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target x86_64-unknown-linux-musl \
    && cp /app/target/x86_64-unknown-linux-musl/release/rust /app/code-push-server

FROM scratch AS runner

WORKDIR /app

COPY --from=builder /app/code-push-server ./code-push-server
COPY public /app/public

ENV PORT=3000

EXPOSE 3000

ENTRYPOINT ["./code-push-server"]