# Running locally on Windows

This runs the same server, scheduler and client as the compose stack, as native
processes on one Windows machine, with no Docker. Everything is served from
`http://localhost:18080` and is reachable only from this PC.

## Prerequisites

Install these once, from an elevated terminal:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install Rustlang.Rustup
winget install OpenJS.NodeJS.22
winget install ShiningLight.OpenSSL.Dev
winget install PostgreSQL.PostgreSQL.18 --override "--mode unattended --superpassword spork --serverport 55432 --disable-components pgAdmin,stackbuilder"
```

- The MSVC build tools are the Rust linker.
- OpenSSL is needed by `webauthn-rs`. The script points `OPENSSL_LIB_DIR` at `C:\Program Files\OpenSSL-Win64\lib\VC\x64\MD` and puts its `bin` directory on `PATH` so the DLLs load.
- On this machine, HTTPS is re-signed by a root certificate that Windows trusts. The script therefore sets `NODE_OPTIONS=--use-system-ca` so npm trusts that root, and `CARGO_HTTP_CHECK_REVOKE=false` because the root's revocation server can't be reached. Certificate verification stays on in both cases.
- PostgreSQL runs as the Windows service `postgresql-x64-18` on port 55432, so the existing connection strings work unchanged.

You also need Git for Windows, because `scripts/migrate.sh` runs under its bash.

## Commands

```powershell
scripts\local.ps1 status    # what is installed, what is pending
scripts\local.ps1 setup     # database, migrations, client bundle, release binaries
scripts\local.ps1 start     # server + scheduler; Ctrl+C stops both
scripts\local.ps1 start -Lan   # the same, reachable from other devices on this network
scripts\local.ps1 firewall  # let other devices in: TCP 18080, Private networks, local subnet (asks for admin)
scripts\local.ps1 seed      # optional demo data (two tenants, password dock-station-1)
scripts\local.ps1 reset     # drop and recreate the local database
```

The first time you run `start` against an empty database, the server logs a setup
token. Use it to create the first administrator. If you run `seed` instead, you
can sign in as `kyle@example.test` / `dock-station-1`.

Passkeys are bound to the relying party `localhost`. A passkey registered here
only works at `http://localhost:18080`, not at `127.0.0.1`.

Runtime state (the setup token and uploaded images) goes in `.local/`, which git
ignores.

After changing client or server code, run `setup` again to rebuild. It is
incremental.

## The face-finder's model

`npm run build` (and so `local.ps1 setup`) downloads the model that finds a
box's face, about 14 MB, from Hugging Face. It comes from a pinned commit and
is checked against its SHA-256 before it is kept, in
`client/public/assets/models/`, which git ignores. A later build finds it
there and downloads nothing.

## Other devices on the network

`start -Lan` listens on every interface and prints the addresses other devices
can use, such as `http://192.168.1.20:18080`. Run `firewall` once to allow them in.
The rule only applies on networks Windows treats as **Private**, and only accepts
connections from the local subnet. If the warehouse WiFi is set to Public, the
command tells you.

Two limits apply until the site has a certificate:

- **Sign in with a password on other devices.** Passkeys only work at
  `localhost`, because browsers require HTTPS for them anywhere else.
- **Traffic is not encrypted.** Passwords cross the WiFi in plain text. That's
  why LAN mode is opt-in.
- **The face-finder runs on one thread.** It finds a box's face in the crop
  screen (D177). Browsers give a page several threads only on HTTPS or at
  localhost, so a phone reaching this PC by address waits several seconds
  for its first answer. A tap to look again is under a second either way.

A page on plain HTTP at an address is not what browsers call a secure context,
so they leave out what they offer only over HTTPS. **Nothing in the client may
depend on one**: `crypto.randomUUID` once made every press on a phone fail
with "Request failed." before anything was sent. An id comes from `uuid()` in
`client/domain/acts.ts`, and `npm run laws` refuses `crypto.randomUUID` and
`crypto.subtle` anywhere in the client.

## Loading NetSuite exports

The local database can hold the business's own data, read from NetSuite exports
and never written back. Each importer is a dry run by default: it prints what it
would write and writes nothing until you add `--apply`. Re-running one on a new
export is safe, since each replays or replaces what the last one wrote.

```sh
# The item master and the prepack list: items, families, box types, cartons.
cargo run -p spork-server --example import_prepack --     --tenant <tenant> --person <person> --site <site>     --items items.csv --prepack prepack.csv [--not-a-box NAME ...]

# One warehouse's bins, from the bin list.
cargo run -p spork-server --example import_bins --     --tenant <tenant> --recorded-by <person> --bins bins.csv --only "<warehouse>"

# Where NetSuite says each thing is: the inventory balance.
cargo run -p spork-server --example import_stock --     --tenant <tenant> --recorded-by <person> --stock balance.csv     --as-at 2026-09-30T09:10:00+10:00 --source netsuite-inventory-balance
```

- The ids are rows in the local database: the tenant, a person in it, and the
  site.
- `--not-a-box` leaves out a name the prepack report would make a box type when
  you know it's a product's carton.
- `--as-at` is when the export was taken. It is required, because a balance
  that can't say how old it is gets read as current.
- NetSuite's balance is stored as its report (`reported_stock`), never as
  Spork's own stock. Screens show the two side by side.

Then, in Spork:
- **Inventory › Warehouse** drafts the layout from the bin list. Preview it,
  then make the places.
- **Workspace › Packing at <site>** says where the site packs and who owns its
  stock. The pack bench won't start a carton until both are set.

## NetSuite: sending picks to Spork

The **Spork Bridge** userscript (in `warehouse-scripts`) keeps Spork's packing
queue fed while a NetSuite tab is open. Once a minute it reads the item
fulfilments the handheld has marked Picked at your location, and sends each to
`POST /api/import/fulfilment`. Nobody opens an item fulfilment to sync it.

1. In Spork, open **Import tokens** (`/tokens`) and mint a token labelled for the
   userscript. Copy it: it is shown once.
2. Install Spork Bridge in Tampermonkey. From the Tampermonkey menu on a NetSuite
   page, set the Spork address (`http://localhost:18080` on this PC, or the LAN
   address), paste the token, and set the location to sync. The first time,
   Tampermonkey asks to allow the connection.

While it works, nothing shows on the page. The first line of the Tampermonkey
menu says when it last synced, and choosing it syncs now. If it can't sync
(Spork unreachable, a refused token, not set up), a small red dot appears in the
bottom-left corner. Hover over it for why, or click it to try again.

Only one NetSuite tab syncs at a time, and moving between NetSuite screens hands
the job on without a gap. Sending the same fulfilment again writes nothing. Items
Spork hasn't seen are created from the fulfilment's code and description.
