# Spork

A warehouse management system for pallet operations: receiving, put away,
picking and despatch, recorded from handheld scanners. Rust and PostgreSQL on
the server, React on the client, Tauri on the handhelds. One deployment serves
more than one company, from the first migration rather than added later.

Every scan writes an event holding what was scanned, the location, the operator
and the time. Those events are the stock record. The movement ledger is append
only, and a scan that disagrees with the record raises a finding with evidence
attached instead of overwriting it.

Spork is built on [Nylonite](https://github.com/kylephillipsau/nylonite), a base
for warehouse systems. Spork's own direction is a local, decentralised system:
every warehouse PC runs a full node, the nodes sync with each other on the site's
network, and it connects to NetSuite through a userscript. See D170 in
[docs/domain-model.md](docs/domain-model.md).

## Requirements

- Rust (stable) and Cargo
- Node 22 and npm
- Docker, for PostgreSQL 18

## Running it

```sh
docker compose up -d postgres              # PostgreSQL on :55432
scripts/migrate.sh                         # apply pending migrations
psql "$DATABASE_URL" -f fixtures/seed.sql  # a small synthetic tenant

cargo run -p spork-server                  # API on :8080
cargo run -p spork-scheduler               # drains dirty projections

cd client && npm ci && npm run dev         # client on :5173
```

`DATABASE_URL` defaults to
`postgres://postgres:spork@localhost:55432/spork`.

On Windows without Docker, `scripts\local.ps1` does all of this natively. See
[docs/local.md](docs/local.md).

## Testing

`DATABASE_URL` is required and its absence is silent: every database-backed test
returns early when it is unset, so an unset variable reports a green suite that
touched nothing.

```sh
scripts/verify-migrations.sh               # every migration up and down
DATABASE_URL=... cargo test --workspace
cd client && npm run verify
```

Build the database fresh for a run. The suite is not re-runnable against one it
has already written to.

## Layout

| Path | Holds |
|---|---|
| `crates/server` | HTTP API, importers, domain modules |
| `crates/invariants` | the invariant suite, run as tests |
| `crates/policy` | the policy resolver |
| `client` | React client, design system, fixture screens |
| `migrations` | schema, each reversible |
| `fixtures` | synthetic seed and history |
| `mobile` | Tauri shell around the client bundle |
| `docs` | the design record |

## The design record

`docs/` holds the reasoning rather than the instructions: the domain decision
record, the invariant and open-question registers, and the analyses behind them.
Start at [docs/architecture.md](docs/architecture.md).

## Licence

MIT. See [LICENSE](LICENSE).
