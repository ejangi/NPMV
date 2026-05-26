# Builder stage
FROM rust:1.95.0-slim-bookworm AS builder
WORKDIR /app

# Copy Cargo configuration files first to cache dependencies
COPY Cargo.toml ./

# Create dummy main.rs to compile dependencies for caching
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -f target/release/deps/npmv*

# Copy the real source files
COPY src ./src

# Rebuild the application with actual source code
RUN cargo build --release

# Runner stage
FROM debian:bookworm-slim
# Install ca-certificates (essential for HTTPS requests to NPM registry)
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/npmv /app/npmv

# Set environment variables for Rocket/Cloud Run
ENV PORT=8080
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=8080

EXPOSE 8080

CMD ["/app/npmv"]
