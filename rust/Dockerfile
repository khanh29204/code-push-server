# Stage 1: Build the Rust application
FROM rust:1.90-slim-bookworm AS builder

# Set the working directory
WORKDIR /app

# Install necessary build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy the Cargo files (assuming context is the project root)
COPY Cargo.toml Cargo.lock ./

# Copy the source code
COPY src ./src

# Build the application in release mode
RUN cargo build --release

# Stage 2: Create the final runtime image
FROM debian:bookworm-slim AS runner

# We set WORKDIR to /app/rust so that when the server runs,
# the path `../public` resolves to `/app/public` where we will copy the UI files.
WORKDIR /app

# Install necessary runtime dependencies (ca-certificates for HTTPS requests)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/rust ./code-push-server

# Copy the public directory for the Web UI
COPY public /app/public

# Set default environment variables
ENV PORT=3000
# Do not set DATABASE_URL or STORAGE_DIR here to allow config.rs to fallback to Node.js env vars

# Expose the application port
EXPOSE 3000

# Use ENTRYPOINT instead of CMD so that any `command:` passed by docker-compose is treated as ignored arguments
ENTRYPOINT ["./code-push-server"]
