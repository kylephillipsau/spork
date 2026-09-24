<#
.SYNOPSIS
    Run Spork natively on this Windows machine, without Docker.

.DESCRIPTION
    The compose stack, but as native processes against a PostgreSQL 18 service
    on localhost:55432. The server serves the built client from the same origin
    as the API, because the session is a cookie (see crates/server/src/assets.rs),
    so everything is at http://localhost:18080 and passkeys are bound to
    "localhost", which browsers accept without TLS.

      scripts\local.ps1 setup     create the database, migrate, build client and binaries
      scripts\local.ps1 start     run the server and the scheduler (Ctrl+C stops both)
      scripts\local.ps1 migrate   apply pending migrations only
      scripts\local.ps1 seed      load fixtures/seed.sql (demo tenants; password dock-station-1)
      scripts\local.ps1 reset     drop and recreate the local database, then migrate
      scripts\local.ps1 status    say what is installed and what is pending

    State the server writes (setup token, uploaded images) lives in .local\,
    which is ignored by git.
#>
param(
    [Parameter(Position = 0)]
    [ValidateSet('setup', 'start', 'migrate', 'seed', 'reset', 'status')]
    [string]$Command = 'status'
)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
$Local = Join-Path $Root '.local'

# ---------------------------------------------------------------------------
# Environment
# ---------------------------------------------------------------------------

$DbName = 'spork'
$DbAdmin = 'postgres://postgres:spork@localhost:55432/postgres'
$env:DATABASE_URL = "postgres://postgres:spork@localhost:55432/$DbName"
$env:BIND = '127.0.0.1:18080'
$env:SPORK_RP_ID = 'localhost'
$env:SPORK_RP_ORIGIN = 'http://localhost:18080'
$env:SPORK_CLIENT_DIR = Join-Path $Root 'client\dist'
$env:SPORK_STATE_DIR = Join-Path $Local 'state'
$env:SPORK_IMAGE_DIR = Join-Path $Local 'images'
$env:SCHEDULER_INTERVAL_SECS = '5'
if (-not $env:RUST_LOG) { $env:RUST_LOG = 'info' }

# Fresh installs put these on the machine PATH, which an already-open terminal
# has not seen. Add them here rather than asking for a new shell.
$extraPaths = @(
    "$env:USERPROFILE\.cargo\bin",
    "$env:ProgramFiles\nodejs",
    "$env:ProgramFiles\PostgreSQL\18\bin",
    "$env:ProgramFiles\OpenSSL-Win64\bin"
)
foreach ($p in $extraPaths) {
    if ((Test-Path $p) -and ($env:Path -split ';' -notcontains $p)) { $env:Path = "$p;$env:Path" }
}

# webauthn-rs links OpenSSL. On Windows there is no system copy, so point the
# build at the ShiningLight install, whose import libraries sit under
# lib\VC\x64\MD rather than where openssl-sys looks. The DLLs it links against
# are found at run time through the bin directory added to PATH above.
$openssl = "$env:ProgramFiles\OpenSSL-Win64"
if ((Test-Path $openssl) -and -not $env:OPENSSL_LIB_DIR) {
    $env:OPENSSL_LIB_DIR = "$openssl\lib\VC\x64\MD"
    $env:OPENSSL_INCLUDE_DIR = "$openssl\include"
}

# Same interception, seen by cargo's schannel: the re-signing root has no
# reachable revocation endpoint (CRYPT_E_NO_REVOCATION_CHECK). The chain is still
# verified; only the revocation lookup is skipped.
if (-not $env:CARGO_HTTP_CHECK_REVOKE) { $env:CARGO_HTTP_CHECK_REVOKE = 'false' }

# This machine's HTTPS is re-signed by a root that Windows trusts and Node's own
# bundle does not, so npm fails with UNABLE_TO_VERIFY_LEAF_SIGNATURE. Trusting
# the system store keeps verification on, unlike strict-ssl=false.
if ($env:NODE_OPTIONS -notmatch 'use-system-ca') { $env:NODE_OPTIONS = "$env:NODE_OPTIONS --use-system-ca".Trim() }

$Bash = "$env:ProgramFiles\Git\bin\bash.exe"

# Judge native tools by exit code alone. When this script's output is redirected,
# Windows PowerShell 5.1 wraps each stderr line in an ErrorRecord, and cargo
# reports progress on stderr — so under 'Stop' a clean build would abort.
function Invoke-Checked([string]$what, [scriptblock]$block) {
    $ErrorActionPreference = 'Continue'
    & $block
    if ($LASTEXITCODE -ne 0) { throw "$what failed (exit $LASTEXITCODE)" }
}

function Assert-Tool([string]$name) {
    if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
        throw "$name is not installed or not on PATH. See docs/local.md."
    }
}

# ---------------------------------------------------------------------------
# Steps
# ---------------------------------------------------------------------------

function Initialize-Database {
    $ErrorActionPreference = 'Continue'   # native stderr is not a failure; see Invoke-Checked
    Assert-Tool psql
    $exists = psql $DbAdmin -tAc "SELECT 1 FROM pg_database WHERE datname = '$DbName'"
    if ($LASTEXITCODE -ne 0) { throw "cannot reach PostgreSQL on localhost:55432. Is the postgresql-x64-18 service running?" }
    if ($exists -ne '1') {
        Invoke-Checked 'createdb' { psql $DbAdmin -q -c "CREATE DATABASE $DbName" }
        Write-Host "created database $DbName"
    }
}

function Invoke-Migrate {
    Assert-Tool psql
    # The migrator is POSIX sh; Git for Windows' bash runs it unchanged, so there
    # is one migrator rather than a PowerShell copy that drifts from it.
    Invoke-Checked 'migrate' { & $Bash (Join-Path $Root 'scripts/migrate.sh') }
}

function Build-Client {
    Assert-Tool npm
    Push-Location (Join-Path $Root 'client')
    try {
        if (-not (Test-Path node_modules)) { Invoke-Checked 'npm ci' { npm ci } }
        Invoke-Checked 'client build' { npm run build }
    } finally { Pop-Location }
}

function Build-Server {
    Assert-Tool cargo
    Push-Location $Root
    try {
        Invoke-Checked 'cargo build' {
            cargo build --release -p spork-server --bin spork-server --bin spork-scheduler
        }
    } finally { Pop-Location }
}

function Start-Spork {
    $ErrorActionPreference = 'Continue'   # native stderr is not a failure; see Invoke-Checked
    $bin = Join-Path $Root 'target\release'
    foreach ($b in 'spork-server.exe', 'spork-scheduler.exe') {
        if (-not (Test-Path (Join-Path $bin $b))) { throw "$b is not built. Run: scripts\local.ps1 setup" }
    }
    New-Item -ItemType Directory -Force $env:SPORK_STATE_DIR, $env:SPORK_IMAGE_DIR | Out-Null

    # The server refuses to start ahead of its schema, so say so here instead.
    Invoke-Migrate

    $scheduler = Start-Process (Join-Path $bin 'spork-scheduler.exe') -NoNewWindow -PassThru
    try {
        Write-Host ""
        Write-Host "Spork: http://localhost:18080  (Ctrl+C to stop)"
        Write-Host "On an empty database the server logs a setup token; use it to create the first administrator."
        Write-Host ""
        & (Join-Path $bin 'spork-server.exe')
    } finally {
        if (-not $scheduler.HasExited) { Stop-Process -Id $scheduler.Id -Force }
    }
}

function Show-Status {
    $ErrorActionPreference = 'Continue'   # native stderr is not a failure; see Invoke-Checked
    foreach ($t in 'cargo', 'node', 'npm', 'psql') {
        $c = Get-Command $t -ErrorAction SilentlyContinue
        '{0,-8} {1}' -f $t, $(if ($c) { $c.Source } else { 'MISSING' })
    }
    '{0,-8} {1}' -f 'openssl', $(if ($env:OPENSSL_LIB_DIR) { $env:OPENSSL_LIB_DIR } else { 'MISSING' })
    if (Get-Command psql -ErrorAction SilentlyContinue) {
        $exists = psql $DbAdmin -tAc "SELECT 1 FROM pg_database WHERE datname = '$DbName'" 2>$null
        if ($LASTEXITCODE -ne 0) { 'database  PostgreSQL not reachable on :55432' }
        elseif ($exists -eq '1') { & $Bash (Join-Path $Root 'scripts/migrate.sh') --status }
        else { 'database  not created (run setup)' }
    }
}

switch ($Command) {
    'setup' {
        Initialize-Database
        Invoke-Migrate
        Build-Client
        Build-Server
        Write-Host "`nReady. Run: scripts\local.ps1 start"
    }
    'start'   { Start-Spork }
    'migrate' { Initialize-Database; Invoke-Migrate }
    'seed'    { Invoke-Checked 'seed' { psql $env:DATABASE_URL -q -v ON_ERROR_STOP=1 -f (Join-Path $Root 'fixtures/seed.sql') } }
    'reset' {
        Assert-Tool psql
        Invoke-Checked 'drop' { psql $DbAdmin -q -c "DROP DATABASE IF EXISTS $DbName WITH (FORCE)" }
        Remove-Item -Recurse -Force $env:SPORK_STATE_DIR -ErrorAction SilentlyContinue
        Initialize-Database
        Invoke-Migrate
    }
    'status' { Show-Status }
}
