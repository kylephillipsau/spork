# Spork

A warehouse management system for pallet operations: receiving, put away,
picking, packing and despatch, recorded from handheld scanners. Rust and PostgreSQL on
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
node scripts/fetch-model.mjs               # once, in client/: the face-finder's model (D177)
```

`npm run build` fetches the model itself; `npm run dev` does not, and without
it the crop screen says its face-finder could not run and the corners are
placed by hand.

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
DATABASE_URL=... cargo test -p spork-server --test it pack_walk_http::   # one file
cd client && npm run verify
```

The server's integration tests are one program, `crates/server/tests/it`, with
one module per file. Tests from different files never run at the same time;
see `file_gate` in `tests/it/common`.

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

## NetSuite

Spork takes work from NetSuite without writing to it. The Spork Bridge
userscript (in the separate `warehouse-scripts` repo) runs while a NetSuite tab
is open, reads NetSuite with SuiteQL as the signed-in user, and sends what it
finds with an import token minted under Import tokens:
- item fulfilments the handheld has marked Picked, to
  `POST /api/import/fulfilment`. Spork records them as picks made elsewhere
  (D172), ready to pack; a kit's own line is no work and its parts are packed
  (D223);
- which of those NetSuite has since packed, shipped or deleted, so Spork
  closes them (D187);
- the inventory balance, every five minutes (D212);
- each item's Pack Unit and supplier part number (D217);
- what NetSuite has still to pick: the goods lines of open sales orders with
  something left to pick, with each order's ship-to and picking instructions,
  every five minutes (D231). Kept as a report, never as work.

It shows nothing unless it can't sync.

The rest comes from NetSuite's CSV exports:
- the item master and the bin list, uploaded under Import;
- the prepack list, and the inventory balance when the Bridge isn't running,
  loaded from the terminal.

NetSuite's balance is kept as its report, beside Spork's own ledger and never
merged into it. [docs/local.md](docs/local.md) has the commands.

Carriers and labels are booked in NetSuite and MachShip. The pack bench plans
the boxes and lists each parcel's size and weight, ready to copy into the
booking (D224).

## Picking from the tickets

Picks are still recorded in NetSuite on the handheld, because Spork doesn't
write to NetSuite yet (D212). Spork guides them. Under Outbound, To pick:
- paste the row of order numbers from the sheet the picking tickets went out
  on, and see which are still waiting, picked, packed or shipped, and where
  each line is picked from (D231);
- share the batch between the people picking, by how many there are and the
  most orders a trip, with a shelf several groups want walked to once (D230);
- print the tickets, one order to an A4 landscape page with its barcode,
  picking instructions and bins in walking order, with a walk sheet on top of
  each trip if wanted;
- walk a trip: each stop in turn, its bin highlighted on the 3D bin map with
  the A* route drawn on the floor, ticked off as it's picked.

## Backups

An administrator downloads a backup under Settings, Backup: one zip of every
row in the workspace, its people and sign-ins, and every photo, encrypted with
a password set there. Restore it into an empty Spork of the same version:

```powershell
scripts\local.ps1 setup
scripts\local.ps1 restore spork-backup-2026-10-02-0900.zip
```

`scripts/backup.sh` is the other kind: a `pg_dump` of the whole database in
the compose stack, for whoever runs the server.

## The design record

`docs/` holds the reasoning rather than the instructions: the domain decision
record, the invariant and open-question registers, and the analyses behind them.
Start at [docs/architecture.md](docs/architecture.md).

Where work stands is in the newest handoff,
[docs/handoff-capture-and-search.md](docs/handoff-capture-and-search.md).

## Licence

MIT. See [LICENSE](LICENSE).
