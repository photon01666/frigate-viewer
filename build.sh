#!/usr/bin/env bash
# Build the Frigate Viewer Android APK.
#
#   ./build.sh              # signed release build (debug keystore)
#   ./build.sh debug        # debuggable build
#   ./build.sh release install
set -euo pipefail
cd "$(dirname "$0")"

MODE="${1:-release}"
INSTALL="${2:-}"
ABI="${ABI:-arm64-v8a}"

case "$MODE" in
    release|debug) ;;
    *) echo "usage: $0 [release|debug] [install]" >&2; exit 2 ;;
esac

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
    if [ -d "$ANDROID_HOME/ndk" ]; then
        ANDROID_NDK_HOME="$(find "$ANDROID_HOME/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)"
    fi
fi
export ANDROID_NDK_HOME

# The Android Gradle Plugin in this project needs a JDK supported by Gradle 8.9.
# Prefer a JDK 21 installation if the shell's default is newer.
if [ ! -x "${JAVA_HOME:-/nonexistent}/bin/java" ] || ! "${JAVA_HOME}/bin/java" -version 2>&1 | grep -qE '"(17|18|19|20|21)\.'; then
    for candidate in \
        "$HOME/.local/share/mise/installs/java/temurin-21" \
        "$HOME/.local/share/mise/installs/java/21.0.9+10.0.LTS" \
        "/usr/lib/jvm/java-21-openjdk"; do
        if [ -x "$candidate/bin/java" ]; then
            export JAVA_HOME="$candidate"
            break
        fi
    done
fi

if [ ! -x "${JAVA_HOME:-/nonexistent}/bin/java" ]; then
    echo "error: a JDK 17-21 with JAVA_HOME is required" >&2
    exit 1
fi
if [ -z "${ANDROID_NDK_HOME:-}" ] || [ ! -d "$ANDROID_NDK_HOME" ]; then
    echo "error: Android NDK not found under $ANDROID_HOME/ndk" >&2
    exit 1
fi

echo "==> Java:   $("$JAVA_HOME/bin/java" -version 2>&1 | head -1)"
echo "==> SDK:    $ANDROID_HOME"
echo "==> NDK:    $ANDROID_NDK_HOME"
echo "==> ABI:    $ABI"
echo "==> Mode:   $MODE"

# Make sure a stale library from a previous ABI or build is not packaged.
rm -rf "android/app/src/main/jniLibs/$ABI"

echo "==> Building Rust library"
if [ "$MODE" = "release" ]; then
    cargo ndk --target "$ABI" --output-dir android/app/src/main/jniLibs build --release --locked
else
    cargo ndk --target "$ABI" --output-dir android/app/src/main/jniLibs build --locked
fi

echo "==> Building APK"
if [ "$MODE" = "release" ]; then
    ./android/gradlew -p android assembleRelease
    APK="android/app/build/outputs/apk/release/app-release.apk"
else
    ./android/gradlew -p android assembleDebug
    APK="android/app/build/outputs/apk/debug/app-debug.apk"
fi

if [ ! -f "$APK" ]; then
    echo "error: expected APK at $APK" >&2
    exit 1
fi

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
mkdir -p dist
OUT="dist/FrigateViewer-${VERSION}-${MODE}.apk"
cp "$APK" "$OUT"
cp "$APK" "dist/FrigateViewer.apk"
echo
echo "APK:  $OUT"
echo "Size: $(du -h "$OUT" | cut -f1)"
echo "SHA256: $(sha256sum "$OUT" | cut -d' ' -f1)"

if [ "$INSTALL" = "install" ]; then
    echo "==> Installing on connected device"
    ADB="$ANDROID_HOME/platform-tools/adb"
    "$ADB" install -r "dist/FrigateViewer.apk"
    "$ADB" shell am start -n com.frigateviewer.app/.MainActivity
fi
