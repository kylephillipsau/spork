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
