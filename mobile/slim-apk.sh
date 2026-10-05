#!/bin/sh
# The debug APK, with the debug symbols taken out of the app's library.
#
# A debug build's library carries every crate's debug info: 198 MB of a
# 400 MB APK, which took long enough over Tailscale that the tablet switched
# wireless debugging off partway through an install. Stripped, the APK is
# about 45 MB and installs in seconds. It is signed again with the same SDK
# debug key, so `adb install -r` still updates the app in place.
#
#   mobile/slim-apk.sh            writes mobile/spork-debug.apk
#   mobile/slim-apk.sh <out.apk>
set -eu
HERE="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
SDK="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
JAVA_HOME="${JAVA_HOME:-/Applications/Android Studio.app/Contents/jbr/Contents/Home}"
export JAVA_HOME PATH="$JAVA_HOME/bin:$PATH"
BT="$(ls -d "$SDK"/build-tools/* | sort -V | tail -1)"
NDK="${NDK_HOME:-$(ls -d "$SDK"/ndk/* | sort -V | tail -1)}"
STRIP="$(ls "$NDK"/toolchains/llvm/prebuilt/*/bin/llvm-strip | head -1)"
IN="$HERE/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk"
OUT="${1:-$HERE/spork-debug.apk}"
LIB=lib/arm64-v8a/libspork_mobile_lib.so

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cp "$IN" "$WORK/in.apk"
cd "$WORK"
unzip -q "$WORK/in.apk" "$LIB"
"$STRIP" --strip-all "$LIB"
# The old signature goes with the old library. The library is stored, not
# deflated, and page-aligned, so Android can map it straight from the APK.
zip -q -d in.apk 'META-INF/*' "$LIB"
zip -q -0 in.apk "$LIB"
"$BT/zipalign" -P 16 -f 4 in.apk aligned.apk
"$BT/apksigner" sign --ks "$HOME/.android/debug.keystore" --ks-pass pass:android \
    --key-pass pass:android --ks-key-alias androiddebugkey --out "$OUT" aligned.apk
"$BT/apksigner" verify "$OUT"
ls -lh "$OUT" | awk '{print $5, $NF}'
