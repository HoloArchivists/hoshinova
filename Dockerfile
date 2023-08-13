# Cache dependencies
FROM node:16 as web-deps
WORKDIR /src/web
COPY web/package.json web/yarn.lock ./
RUN yarn install --frozen-lockfile

# Create base image for building Rust
FROM rust:1.62-alpine AS rust-build-image
RUN apk add --no-cache musl-dev git

# Cache dependencies
FROM rust-build-image AS rust-deps
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
RUN set -ex; \
    mkdir src; \
    echo 'fn main() {}' > src/main.rs; \
    cargo build --locked --release --target x86_64-unknown-linux-musl; \
    rm -rf src;

# Generate TypeScript bindings
FROM rust-build-image AS ts-bind
WORKDIR /src
COPY --from=rust-deps /usr/local/cargo /usr/local/cargo
COPY . .
RUN set -ex; \
    touch src/main.rs; \
    cargo test

# Build the web app
FROM node:16 AS web-builder
WORKDIR /src/web
COPY web .
COPY --from=web-deps /src/web/node_modules /src/web/node_modules
COPY --from=ts-bind /src/web/src/bindings /src/web/src/bindings
RUN yarn build

# Build the Rust app
FROM rust-build-image AS rust-builder
WORKDIR /src
COPY --from=ts-bind /usr/local/cargo /usr/local/cargo
COPY --from=ts-bind /src /src
COPY --from=rust-deps /src/target /src/target
COPY --from=web-builder /src/web/dist /src/web/dist
RUN touch src/main.rs && \
    cargo build --locked --release --target x86_64-unknown-linux-musl

# Build ytarchive
FROM golang:1.20-alpine AS ytarchive-builder
WORKDIR /src
RUN set -ex; \
    apk add --no-cache git; \
    git clone https://github.com/Kethsar/ytarchive.git; \
    cd ytarchive; \
    git checkout v0.3.2; \
    go build .

FROM alpine AS runner
WORKDIR /app
RUN apk add --no-cache ffmpeg
COPY --from=ytarchive-builder /src/ytarchive/ytarchive /usr/local/bin/ytarchive

USER 1000
COPY --from=rust-builder --chown=1000:1000 \
  /src/target/x86_64-unknown-linux-musl/release/hoshinova \
  /app/hoshinova

CMD ["/app/hoshinova"]
