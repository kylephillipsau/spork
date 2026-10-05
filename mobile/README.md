# spork-mobile

The Tauri 2 shell for the floor app. **There is no second interface**: this runs
the same React client the browser runs, built in a mode that points the
transport at an absolute server and a bearer token instead of a same-origin
cookie.

Built locally and installed by hand. There is no store pipeline here and that is
deliberate — see *Shipping it, later*.

## Why a webview shell and not React Native

The client is a large React app on a bespoke design system — 28 design files,
`LightRoom`, four shells, and 68 fixture screens that a render gate visits at
two densities in light and dark. React Native reuses none of that, and the gate
would cover none of the app. What a device actually changes turned out to be
three seams: where the server is, which HTTP client reaches it, and what
credential it carries.

`SignOnResponse.token` has said this was coming since the day it was written —
*"Returned once, for clients that cannot hold a cookie — D5's handhelds. A
browser ignores this and uses the cookie."* D165.

## Layout

| | |
|---|---|
| `src-tauri/` | the Rust shell. Its own Cargo workspace, so the server's CI never builds it |
| `src-tauri/gen/` | the generated Xcode and Gradle projects. **Committed**, because they get hand-edited and must be reproducible |
| `client/app/platform/mobile.ts` | the transport binding, in the client rather than here — it is TypeScript the client loads |

## Signing: which account

Nothing here ties this app to any developer account. The bundle identifier is
`com.warehouseutilities.spork`, the team ID is never committed, and both are independent
of whatever else you ship — so putting Spork on its own developer account
later costs one environment variable and nothing else.

Two ways to sign for a device, and the difference is how often you rebuild:

- **A free personal team** signs and installs on your own phone with no paid
  membership. Profiles expire **seven days** from issue, so the app stops
  launching about a week after each build and you reinstall. There are also caps
  — 10 App IDs and 3 devices, each on the same seven-day clock.
- **A paid membership** ($99/yr) takes that to roughly a year, which is the
  point at which you stop thinking about it.

For the first pass on your own phone, the free team is enough and a week is
usually longer than the code stays interesting.

```sh
security find-identity -v -p codesigning   # the team IDs available to you
export APPLE_DEVELOPMENT_TEAM=<team id>
```

Not committed, for the reason it is not a property of the software. Either the
environment variable, or a gitignored `src-tauri/tauri.ios.conf.json`.

## Build and install

**From your own terminal**, which is the point: a Terminal or iTerm shell is in
the `Aqua` session, so `codesign` can reach the key. Confirm with
`launchctl managername` — it should print `Aqua` and not `Background`.

```sh
export APPLE_DEVELOPMENT_TEAM=<team id>
npm --prefix mobile install
npm --prefix client run build:mobile
npm --prefix mobile run tauri -- ios init      # once, and after a config change
npm --prefix mobile run ios:build              # --debug --export-method debugging

xcrun devicectl list devices
xcrun devicectl device install app --device <UUID> \
  mobile/src-tauri/gen/apple/build/arm64/Spork.ipa
```

The bundle is named after `productName` in `tauri.conf.json` — `Spork.ipa`,
not `spork-mobile.ipa`, which is the Cargo package and the Xcode project.
Installing does not need a signature, so that last step runs from anywhere.

`--export-method debugging` produces a standalone app. `tauri ios dev` instead
tethers the webview to the laptop's vite server, so the app stops working the
moment the laptop does — which is not a floor test, but is the fastest loop for
changing a screen. `beforeDevCommand` passes `--mode mobile`; without it vite
serves the default mode, the transport never rebinds, and the app asks the vite
origin for `/api` and gets nothing.

## Android (a Boox Tab Ultra, first)

No signing account is involved: a debug build is signed with the SDK's debug
key, so the whole thing runs from a shell. Android Studio's own Java and SDK
are enough:

```sh
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
export ANDROID_HOME="$HOME/Library/Android/sdk"
export NDK_HOME="$ANDROID_HOME/ndk/<version>"
export PATH="$ANDROID_HOME/platform-tools:$PATH"
# Its own cargo folder: a shared one is locked by whatever else is building.
export CARGO_TARGET_DIR="$HOME/.cargo/spork-mobile-target"

npm --prefix mobile install
cd mobile && npx tauri android build --debug --target aarch64 --apk
```

The Tab Ultra is `arm64-v8a` on Android 11, so `--target aarch64` is all it
needs. The APK lands under `src-tauri/gen/android/app/build/outputs/apk/`.

**Then slim it: `mobile/slim-apk.sh`.** A debug build's library keeps every
crate's debug symbols, about 400 MB of APK. Over Tailscale that took long
enough for the tablet to switch wireless debugging off mid-install. The script
strips them and signs it again with the same debug key, writing
`mobile/spork-debug.apk` (about 45 MB). Install that one.

The app is not drawn edge to edge (`MainActivity.kt`): under the Boox's
status bar, the header and its account menu were hidden.

**Installing over Tailscale.** On the tablet, Settings › Developer options ›
Wireless debugging. Pair once with the code it shows, then connect to the
port it shows now. Use the tablet's **Tailscale** address, not the wifi one the
screen prints; the port is the same either way. The port changes whenever
wireless debugging is switched off, which Android does by itself now and then.

```sh
adb pair <tailscale-ip>:<pairing-port> <code>     # once
adb connect <tailscale-ip>:<port>
adb -s <tailscale-ip>:<port> install -r mobile/spork-debug.apk
```

**Which server.** The app's sign-in screen asks for it the first time (a
work computer started with `scripts\local.ps1 start -Lan` prints the address
to use, port 18080) and keeps it on the device; **Change** under the form sets
another. When the server can't be reached, every screen says so above it,
with **Try again** and the address to change: the app keeps its sign-in, so it
opens to its screens rather than to sign-in. Plain HTTP is allowed only to
private networks — the warehouse wifi, Tailscale, `.local` names — and HTTPS
anywhere (`capabilities/default.json`).

## Pointing it at a different server

The default is `https://spork.warehouseutilities.com`. For a laptop build:

```sh
VITE_SPORK_SERVER=https://spork.warehouseutilities.com npm --prefix client run build:mobile
```

A build can bake an address in this way; the address set on the device's
sign-in screen outranks it (`client/app/platform/server.ts`).

## First time on the device, in this order

The first two would make everything after them meaningless.

1. **Sign in.** Proves the native HTTP client reaches the server, the bearer is
   attached, and the token is kept.
2. **Sign out.** *Most likely thing to be broken.* `Framed` and the sign-in
   screen navigate with `window.location.assign` — a full page load to an SPA
   path. The deployed server answers those from `assets.rs`'s catch-all; how the
   Tauri asset resolver answers an extensionless path is **not verified**. If
   this lands on a dead page, the fix is a mobile-mode branch in those two
   places assigning `/` instead.
3. **The walk renders** at `/capture`, with bins and stock figures.
4. Bind a barcode from the capture session.
5. Take a photograph. The upload goes through `fetch` and should work; it will
   not draw afterwards — see below.

## Gotchas, most of them Nosdesk's, paid for once already

- **The bundled UI comes from `client/dist-mobile`**, so the app always ships
  what `build:mobile` last produced rather than whatever a dev server has.
- **Touch `src-tauri/src/lib.rs` after a client-only change.** The Rust side
  embeds the assets and will silently ship the previous bundle if it has no
  reason to recompile.
- **Unlock the login keychain** or `CodeSign` fails at the end of an otherwise
  clean build: `security -v unlock-keychain ~/Library/Keychains/login.keychain-db`.
- **Xcode's build environment has almost no `PATH`, and the Rust phase needs
  one.** The `Build Rust Code` phase runs `npm run -- tauri ios xcode-script`,
  which then runs `cargo` — and neither `/opt/homebrew/bin/npm` nor
  `~/.cargo/bin/cargo` is on the `PATH` Xcode gives a script phase. From a
  terminal it inherits yours and works; from the Xcode GUI it fails as
  `Command PhaseScriptExecution failed with a nonzero exit code`, which says
  nothing about why.

  The phase now exports a `PATH` first, in **both** `project.yml` (what
  xcodegen reads) and the generated `project.pbxproj` (what today's build
  reads). `tauri ios init` rewrites both from its own template, so **re-apply
  this after any `init`** — it is two identical edits and this note is the
  reminder.

- **The build must run from a GUI login session, which means an agent cannot do
  it.** Code signing needs the private key out of the login keychain, and only a
  process in the `Aqua` session can have it. A shell that reports
  `launchctl managername` → `Background` gets `errSecInternalComponent` from
  `codesign` however unlocked the keychain is, and unlocking it from another
  terminal does not help — the lock is not the problem, the session is. Checked
  rather than assumed: signing a copy of `/bin/echo` fails the same way, with
  and without the sandbox.

  So everything up to the signature can be automated — the Rust cross-compile,
  the asset catalogue, the plist — and the last step is a person's. Pressing
  **Run** in Xcode with the device selected does build, sign, install and launch
  in one go, which is the shortest path. `xcrun devicectl device install` needs
  no signing, so a already-signed `.app` can be installed by anything.
- **Target the device by UUID.** Device names often contain a curly apostrophe,
  so `--device "Kyle's iPhone"` with a straight quote will not match.

## What does not work yet

- **Photographs will not render.** `GET /api/images/{digest}` is authenticated
  (`routes.rs`: *"the row is what authorises the bytes"*), and an `<img src>` in
  a webview cannot carry a bearer header. Nosdesk answers this with a custom URI
  scheme and a Rust asset proxy (`asset_proxy.rs`); that is the shape this
  needs. Taking a photograph works — it goes through `fetch`.
- **The session token is in `localStorage`.** It *is* the session on a device,
  so that is a credential at rest in the webview's store. Nosdesk uses a
  Keychain/Keystore plugin (189 lines of Rust, ~100 each of Swift and Kotlin)
  and this should too before anybody who is not Kyle installs the app. Said
  again in `client/app/platform/mobile.ts`, where the next person to touch it
  will be.
- **No camera scanner.** That is the next plugin, and the reason for the shell:
  MLKit on Android, Vision on iOS, handing a decoded string to the same
  `onScan` a wedge scanner types into.

## Shipping it, later

There was a fastlane pipeline here for one commit and it is gone. It uploaded to
TestFlight and Play, neither lane had ever been run, and this register refuses
code built against a caller that does not exist — the same rule the work rail
states as *"a greyed row is a promise, and a rail full of promises is a rail
people stop reading."*

When there is a developer account and somebody to test it, `~/dev/Nosdesk/mobile`
has a working pair of lanes against the same two stores. Two things in them were
wrong for this repository and would be again: their `Info.plist` sits in
`app_iOS` because their Cargo package is named `app`, and their `tauri()` helper
omits a prefix, which works under pnpm — it walks up to a workspace root — and
does not under npm, since fastlane's `sh` runs from `fastlane/`.
