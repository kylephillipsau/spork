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
      scripts\local.ps1 start -Lan   the same, reachable from other devices on this network
      scripts\local.ps1 firewall  allow other devices on this network in (asks for admin once)
      scripts\local.ps1 https     fetch Caddy, which serves HTTPS to paired devices with -Lan
      scripts\local.ps1 migrate   apply pending migrations only
      scripts\local.ps1 seed      load fixtures/seed.sql (demo tenants; password dock-station-1)
      scripts\local.ps1 reset     drop and recreate the local database, then migrate
      scripts\local.ps1 status    say what is installed and what is pending
      scripts\local.ps1 restore <backup.zip>   restore a workspace backup into an empty Spork

    With -Lan, the server also keeps this host's identity (D255): its own
    authority and a certificate for the name the computer announces on the
    network (<computer>.local, by mDNS), and Caddy serves HTTPS on that name.
    A device is paired once at http://<computer>.local:18080/pair, and then
    finds and trusts this host wherever its address moves.

    State the server writes (setup token, uploaded images, the identity) lives
    in .local\, which is ignored by git.
#>
param(
    [Parameter(Position = 0)]
    [ValidateSet('setup', 'start', 'migrate', 'seed', 'reset', 'status', 'firewall', 'https', 'restore')]
    [string]$Command = 'status',
    # The backup a restore reads (D193).
    [Parameter(Position = 1)]
    [string]$File,
    # Listen on every interface rather than loopback only. Opt-in, because the
    # server speaks plain HTTP: over the LAN a password crosses the WiFi in the
    # clear until the site has a certificate (see the plan's risks).
    [switch]$Lan
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
$Port = 18080
$env:BIND = if ($Lan) { "0.0.0.0:$Port" } else { "127.0.0.1:$Port" }
$env:SPORK_RP_ID = 'localhost'
$env:SPORK_RP_ORIGIN = 'http://localhost:18080'
$env:SPORK_CLIENT_DIR = Join-Path $Root 'client\dist'
$env:SPORK_STATE_DIR = Join-Path $Local 'state'
$env:SPORK_IMAGE_DIR = Join-Path $Local 'images'

# This host's identity and its HTTPS (D255): found by the name the computer
# announces on the network, trusted through Spork's own authority. Caddy is
# pinned and checked against its release's checksums.
$HttpsPort = 443
$HostName = if ($env:SPORK_HOST_NAME) { $env:SPORK_HOST_NAME } else { "$($env:COMPUTERNAME.ToLower()).local" }
if ($Lan) {
    $env:SPORK_IDENTITY = '1'
    $env:SPORK_HOST_NAME = $HostName
}
$CaddyVersion = '2.11.4'
$Caddy = Join-Path $Local 'caddy\caddy.exe'
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
            cargo build --release -p spork-server --bin spork-server --bin spork-scheduler --bin spork-restore
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
    $https = $Lan -and (Test-Path $Caddy)
    $caddy = $null
    try {
        Write-Host ""
        Write-Host "Spork: http://localhost:$Port  (Ctrl+C to stop)"
        if ($Lan) {
            Write-Host "       http://${HostName}:$Port  (other devices, by name, wherever this one moves)"
            foreach ($ip in Get-LanAddresses) { Write-Host "       http://${ip}:$Port  (by address, until it changes)" }
            if ($https) {
                Write-Host "       https://$HostName  (paired devices; pair one at http://${HostName}:$Port/pair)"
            } else {
                Write-Host "HTTPS is not set up: run scripts\local.ps1 https, then scripts\local.ps1 firewall."
            }
            Write-Host "Passkeys work at localhost only; elsewhere, sign in with a password."
        }
        Write-Host "On an empty database the server logs a setup token; use it to create the first administrator."
        Write-Host ""
        if ($https) {
            $server = Start-Process (Join-Path $bin 'spork-server.exe') -NoNewWindow -PassThru
            # The server makes this host's certificate as it starts; HTTPS waits for it.
            $chain = Join-Path $env:SPORK_STATE_DIR 'identity\host-chain.crt'
            for ($i = 0; $i -lt 60 -and -not (Test-Path $chain) -and -not $server.HasExited; $i++) { Start-Sleep -Milliseconds 500 }
            if (Test-Path $chain) { $caddy = Start-Caddy }
            $server.WaitForExit()
        } else {
            & (Join-Path $bin 'spork-server.exe')
        }
    } finally {
        if ($caddy -and -not $caddy.HasExited) { Stop-Process -Id $caddy.Id -Force }
        if (-not $scheduler.HasExited) { Stop-Process -Id $scheduler.Id -Force }
    }
}

# HTTPS on this host's name, in front of the server (D255), with the
# certificate the server keeps. Its admin endpoint is off: nothing changes it
# while it runs, and a restart reads a renewed certificate.
function Start-Caddy {
    $id = (Join-Path $env:SPORK_STATE_DIR 'identity') -replace '\\', '/'
    $file = Join-Path $Local 'caddy\Caddyfile'
    @(
        '{'
        '	admin off'
        '	auto_https off'
        '	persist_config off'
        '}'
        "https://${HostName}:$HttpsPort {"
        "	tls `"$id/host-chain.crt`" `"$id/host.key`""
        "	reverse_proxy 127.0.0.1:$Port"
        '}'
    ) | Set-Content -Encoding ascii $file
    Start-Process $Caddy -ArgumentList 'run', '--config', "`"$file`"", '--adapter', 'caddyfile' -NoNewWindow -PassThru
}

# Caddy, pinned, and checked against the SHA-512 its release publishes before
# it is unpacked. Into .local\caddy, beside nothing else.
function Install-Caddy {
    if (Test-Path $Caddy) { Write-Host "Caddy $CaddyVersion is installed."; return }
    $dir = Split-Path $Caddy
    New-Item -ItemType Directory -Force $dir | Out-Null
    $base = "https://github.com/caddyserver/caddy/releases/download/v$CaddyVersion"
    $name = "caddy_${CaddyVersion}_windows_amd64.zip"
    $zip = Join-Path $dir $name
    Invoke-WebRequest "$base/$name" -OutFile $zip -UseBasicParsing
    # Served as bytes (application/octet-stream), so read as text first.
    $raw = (Invoke-WebRequest "$base/caddy_${CaddyVersion}_checksums.txt" -UseBasicParsing).Content
    $sums = if ($raw -is [byte[]]) { [Text.Encoding]::UTF8.GetString($raw) } else { $raw }
    $want = (($sums -split "`n") | Where-Object { $_ -match "\s$([regex]::Escape($name))$" } | Select-Object -First 1) -split '\s+' | Select-Object -First 1
    $got = (Get-FileHash $zip -Algorithm SHA512).Hash
    if (-not $want -or $got -ne $want.ToUpper()) {
        Remove-Item $zip -Force
        throw "Caddy's download does not match its published checksum; nothing was installed"
    }
    Expand-Archive $zip -DestinationPath $dir -Force
    Remove-Item $zip -Force
    Write-Host "Caddy $CaddyVersion installed in $dir."
}

# A workspace's backup, put back into an empty Spork (D193). The database is
# brought to this Spork's migration first; the backup must have been taken at
# the same one. The password is asked here and handed over in the environment,
# never on a command line where the process list would show it.
function Restore-Backup {
    $ErrorActionPreference = 'Continue'   # native stderr is not a failure; see Invoke-Checked
    if (-not $File) { throw "Say which backup: scripts\local.ps1 restore <backup.zip>" }
    $path = Resolve-Path $File -ErrorAction Stop
    $exe = Join-Path $Root 'target\release\spork-restore.exe'
    if (-not (Test-Path $exe)) { throw "spork-restore.exe is not built. Run: scripts\local.ps1 setup" }
    New-Item -ItemType Directory -Force $env:SPORK_IMAGE_DIR | Out-Null
    Initialize-Database
    Invoke-Migrate
    $secret = Read-Host -AsSecureString 'Backup password'
    $env:SPORK_BACKUP_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringBSTR(
        [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secret))
    try {
        Invoke-Checked 'restore' { & $exe $path }
    } finally {
        Remove-Item Env:SPORK_BACKUP_PASSWORD -ErrorAction SilentlyContinue
    }
    Write-Host "`nRestored. Run: scripts\local.ps1 start"
}

# IPv4 addresses other devices can reach: DHCP or manual, not loopback or
# link-local (169.254.x.x means the adapter never got an address).
function Get-LanAddresses {
    Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
        Where-Object { $_.PrefixOrigin -in 'Dhcp', 'Manual' -and $_.IPAddress -notlike '169.254.*' } |
        Select-Object -ExpandProperty IPAddress
}

# One inbound rule, scoped as narrowly as the job allows: this port, TCP, the
# Private profile only, and only from the local subnet. A laptop that joins a
# café's WiFi (Public) is not serving the warehouse to strangers.
function Open-Firewall {
    $rule = 'Spork (LAN)'
    $admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).
        IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if (-not $admin) {
        Write-Host "Asking Windows for admin rights to add the firewall rule..."
        $p = Start-Process powershell -Verb RunAs -Wait -PassThru -ArgumentList @(
            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $PSCommandPath, 'firewall')
        if ($p.ExitCode -ne 0) { throw "the firewall rule was not added (exit $($p.ExitCode))" }
    } else {
        Get-NetFirewallRule -DisplayName $rule -ErrorAction SilentlyContinue | Remove-NetFirewallRule
        New-NetFirewallRule -DisplayName $rule -Direction Inbound -Action Allow -Protocol TCP `
            -LocalPort $Port -Profile Private -RemoteAddress LocalSubnet | Out-Null
        # HTTPS for paired devices (D255), as narrow.
        Get-NetFirewallRule -DisplayName 'Spork (HTTPS)' -ErrorAction SilentlyContinue | Remove-NetFirewallRule
        New-NetFirewallRule -DisplayName 'Spork (HTTPS)' -Direction Inbound -Action Allow -Protocol TCP `
            -LocalPort $HttpsPort -Profile Private -RemoteAddress LocalSubnet | Out-Null
    }
    Write-Host "Firewall: TCP $Port and $HttpsPort open to the local subnet on Private networks."
    $public = Get-NetConnectionProfile -ErrorAction SilentlyContinue | Where-Object NetworkCategory -eq 'Public'
    foreach ($n in $public) {
        Write-Host "  '$($n.Name)' is set to Public, so the rule does not apply there. If it is the"
        Write-Host "  warehouse network, set it to Private in Settings > Network & internet."
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
        Install-Caddy
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
    'firewall' { Open-Firewall }
    'https'   { Install-Caddy }
    'restore' { Restore-Backup }
}
