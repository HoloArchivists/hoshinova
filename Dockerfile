FROM node:16 AS web-builder

# Cache dependencies
WORKDIR /src
COPY web/package.json web/yarn.lock ./
RUN yarn install

# Build
COPY web .
RUN yarn build

FROM rust:1.62-alpine AS rust-builder
RUN apk add --no-cache musl-dev

# Cache dependencies
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
RUN set -ex; \
    mkdir src; \
    echo 'fn main() {}' > src/main.rs; \
    cargo build --release --target x86_64-unknown-linux-musl; \
    rm -rf src;

# Build
COPY . .
COPY --from=web-builder /src /src/web
RUN touch src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl

FROM alpine AS runner
WORKDIR /app
RUN set -ex; \
    apk add --no-cache ffmpeg wget unzip; \
    wget -O /app/ytarchive.zip https://github.com/Kethsar/ytarchive/releases/download/latest/ytarchive_linux_amd64.zip; \
    unzip /app/ytarchive.zip -d /usr/local/bin/; \
    rm /app/ytarchive.zip; \
    apk del wget unzip;

USER 1000
COPY --from=rust-builder --chown=1000:1000 \
  /src/target/x86_64-unknown-linux-musl/release/hoshinova \
  /app/hoshinova

CMD ["/app/hoshinova"]
