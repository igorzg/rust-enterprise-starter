# ---------- builder ----------
FROM rust:1.96-slim AS builder

WORKDIR /app

# utoipa-swagger-ui's build script downloads the Swagger UI bundle with the
# system `curl` at compile time (the `reqwest` feature is not enabled), so
# the builder stage needs it. The runtime image does not.
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*

# First pass: compile dependencies only, against a minimal source tree,
# so the dependency cache survives changes to src/.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && printf 'fn main() {}\n' > src/main.rs \
    && printf '' > src/lib.rs \
    && cargo build --release

# Second pass: compile the real application.
COPY src ./src
# BuildKit COPY preserves host mtimes, which are older than the first-pass
# build fingerprint, so cargo would skip the rebuild and keep the stub
# binary. Touching the sources forces the real crate to recompile.
RUN find src -type f -name '*.rs' -exec touch {} + \
    && cargo build --release

# ---------- runtime ----------
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 app

USER app
WORKDIR /app

COPY --from=builder /app/target/release/rstarter /usr/local/bin/rstarter

ENV APP_HOST=0.0.0.0 \
    APP_PORT=8080

EXPOSE 8080

CMD ["rstarter"]
