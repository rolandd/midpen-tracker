# Build stage - use Rust with musl for static binary
FROM rust:1.92-alpine AS dependencies

# Install build dependencies for musl
RUN apk add --no-cache musl-dev

# Set up workdir
WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./

# Remove [[bench]] section to avoid needing actual benchmark files
# Logic: Find range from [[bench]] to next [section]. Inside range: delete [[bench]] header, delete non-[ header lines.
RUN sed -i '/^\[\[bench\]\]/,/^\[/ { /^\[\[bench\]\]/d; /^\[/!d; }' Cargo.toml

# Create dummy main.rs to build dependencies
RUN mkdir src && echo 'fn main() {}' > src/main.rs && echo 'pub fn lib() {}' > src/lib.rs

# Build dependencies only (cached layer)
RUN cargo build --release && rm -rf src

# --- End of cached dependencies stage ---

FROM dependencies AS builder

# Copy actual source code
COPY src ./src
COPY data ./data
COPY permissions-policy.txt ./

ARG BUILD_ID=unknown
ENV BUILD_ID=${BUILD_ID}

# Build the real binary
RUN touch src/main.rs src/lib.rs && cargo build --release

# Runtime stage - minimal distroless image
FROM gcr.io/distroless/static-debian13:nonroot

# Copy the binary
COPY --from=builder /app/target/release/midpen-tracker /app/midpen-tracker

# Copy preserve boundaries (needed at runtime)
COPY --from=builder /app/data /app/data

# Set workdir
WORKDIR /app

# Run as non-root user (distroless:nonroot default UID is 65532)
USER nonroot

# Expose port (Cloud Run uses PORT env var)
EXPOSE 8080

# Run the binary
ENTRYPOINT ["/app/midpen-tracker"]
