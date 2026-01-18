# Dockerfile for Voice Stream WebRTC Add-on

ARG BUILD_FROM=ghcr.io/home-assistant/amd64-base:latest

# ------------------------------------------------------------------------------
# Build Stage
# ------------------------------------------------------------------------------
FROM rust:1.92.0-alpine3.23 AS builder

WORKDIR /usr/src/app

# Install build dependencies
RUN apk add --no-cache \
    pkgconfig \
    openssl-dev \
    musl-dev \
    cmake \
    g++ \
    git \
    make

# 1. Copy only the dependency manifests
COPY Cargo.toml Cargo.lock ./

# 2. Build a dummy main.rs to compile and cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src/

# 3. Now copy the REAL source code
COPY src ./src

# 4. Build the actual binary (this will be fast as dependencies are already built)
RUN cargo build --release && \
    strip target/release/webrtc_server

# ------------------------------------------------------------------------------
# Runtime Stage
# ------------------------------------------------------------------------------
FROM $BUILD_FROM

# Install runtime dependencies (e.g. libssl, openssl CLI)
RUN \
    apk add --no-cache \
    libgcc \
    libssl3 \
    libcrypto3 \
    openssl \
    bash

WORKDIR /app

# Copy the binary from the builder stage
COPY --from=builder /usr/src/app/target/release/webrtc_server /app/webrtc_server

# Copy certificate generator
COPY generate_lan_cert.sh /usr/bin/generate_lan_cert.sh
RUN chmod +x /usr/bin/generate_lan_cert.sh

# Create S6 service
COPY rootfs /

# Make binary executable (should be already)
RUN chmod +x /app/webrtc_server

# Ensure /app/ssl exists (symlink or dir) for the app to find certs
# In HA, certs are in /ssl. The app looks for ./ssl/cert.pem
# We can create a symlink /app/ssl -> /ssl
RUN ln -s /ssl /app/ssl

# Note: The S6 run script (in rootfs/etc/services.d/webrtc/run) will start the app
