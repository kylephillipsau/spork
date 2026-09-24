# Workspace binaries for local compose: API server and projection drain.
# Shared build stage so both images compile the crate once.
#
# The server image also carries the client bundle, because the session is a
# cookie and the bundle has to answer from the same origin as the API. See
# crates/server/src/assets.rs.

# ---------------------------------------------------------------------------
# The client. Dependencies first, so a source edit does not reinstall npm.
# ---------------------------------------------------------------------------
FROM node:22-bookworm-slim AS client
WORKDIR /client

COPY client/package.json client/package-lock.json ./
RUN npm ci

COPY client/ ./
# `build` is `tsc --noEmit && vite build`, so a type error fails the image.
#
# The other three gates — the token contract, the client/server type contract,
# and the design laws — run in CI rather than here. They check that the source
# is coherent, which is a property of a commit; this stage turns a commit that
# already passed them into an artefact. Running them twice would also mean
# copying `crates/` into a node image to satisfy the contract check, which is a
# coupling worth not having.
RUN npm run build

# ---------------------------------------------------------------------------
# The binaries.
# ---------------------------------------------------------------------------
FROM rust:1-bookworm AS build
WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# **The commit, so the binary can say which build it is** (D163). A deployment
# that cannot answer that turns "did the deploy land?" into a question only
# somebody with SSH can settle, which is the state this was added from.
#
# It sits after the source copy on purpose: that layer is already invalidated by
# every commit, so carrying the sha costs nothing. Put it earlier and it would
# bust the dependency layer as well, for a string.
#
# Empty in a local build, which `option_env!` reads as None and the endpoint
# reports as null rather than inventing a version.
ARG GIT_SHA=""
ENV SPORK_GIT_SHA=$GIT_SHA

RUN cargo build -p spork-server --release \
        --bin spork-server \
        --bin spork-scheduler \
 && strip target/release/spork-server target/release/spork-scheduler

FROM debian:bookworm-slim AS runtime-base
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3 \
 && rm -rf /var/lib/apt/lists/*
ENV RUST_LOG=info
ENV DATABASE_URL=postgres://postgres:spork@postgres:5432/spork

FROM runtime-base AS server
COPY --from=build /src/target/release/spork-server /usr/local/bin/spork-server
COPY --from=client /client/dist /usr/local/share/spork/client
ENV SPORK_CLIENT_DIR=/usr/local/share/spork/client
# Listen on all interfaces inside the container (host maps 8080).
ENV BIND=0.0.0.0:8080
EXPOSE 8080
ENTRYPOINT ["spork-server"]

# The scheduler serves nothing and carries no bundle.
FROM runtime-base AS scheduler
COPY --from=build /src/target/release/spork-scheduler /usr/local/bin/spork-scheduler
ENV SCHEDULER_INTERVAL_SECS=5
ENTRYPOINT ["spork-scheduler"]

# ---------------------------------------------------------------------------
# The migrator, which runs to completion before the server starts.
# ---------------------------------------------------------------------------
#
# **Ordering rather than assertion.** The server has no boot-time schema check
# and does not need one: this runs first and the server waits on it, so a binary
# ahead of its schema is not a state the deployment can be in. That failure
# shipped twice — a stack restarted against a database missing migration 78, a
# clean start in the log, and an endpoint answering 500 behind a green health
# check — and both times the fix was a person remembering.
#
# It carries `psql` and the SQL rather than the Rust binaries: a migration is a
# file this applies, and building the whole workspace to run `psql` would tie
# the schema's deploy to the code's compile.
FROM debian:bookworm-slim AS migrate
RUN apt-get update \
 && apt-get install -y --no-install-recommends postgresql-client ca-certificates \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY migrations/ migrations/
COPY scripts/migrate.sh scripts/migrate.sh
RUN chmod +x scripts/migrate.sh
ENTRYPOINT ["scripts/migrate.sh"]
