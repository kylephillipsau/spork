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

## NetSuite: sending fulfilments to Spork

The **Spork Sync** userscript (in `warehouse-scripts`) adds a *Send to Spork*
button to NetSuite's Item Fulfillment page. It sends the lines marked to fulfil
to `POST /api/import/fulfilment`, and Spork loads them as work to pick.

1. In Spork, open **Import tokens** (`/tokens`) and mint a token labelled for the
   userscript. Copy it: it is shown once.
2. Install Spork Sync in Tampermonkey. From the Tampermonkey menu on a NetSuite
   page, set the Spork address (`http://localhost:18080` on this PC, or the LAN
   address) and paste the token.
3. Open an Item Fulfillment and click *Send to Spork*. The first time, Tampermonkey
   asks to allow the connection.

Sending the same fulfilment again writes nothing. If a quantity has changed since
the last send, Spork reports the difference and leaves its copy as it was. Items
Spork hasn't seen are created from the page's code and description. Lines at a
warehouse Spork doesn't know are skipped and listed in the reply.
