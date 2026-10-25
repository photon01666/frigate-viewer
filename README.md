# Frigate Viewer

A small Android app that wraps the Frigate NVR web UI in a native WebView shell.
Built in Rust (config, local setup server, and connection checks) plus a thin Java
WebView host.

## Features

- First-run setup page for your Frigate server URL
- Full Frigate web UI (live view, events, recordings, login) inside the app
- HTTPS with self-signed certificates (opt-in)
- Keep-screen-awake while viewing
- Floating settings button overlaid on the Frigate UI
- Frigate bird as the launcher icon

## Build

Requirements: Rust with the `aarch64-linux-android` target, JDK 21, Android SDK
(platform 34 + build-tools 34), Android NDK 27, and `cargo-ndk`.

```sh
rustup target add aarch64-linux-android
./build.sh            # release
./build.sh debug      # debuggable
./build.sh release install
```

The APK is written to `dist/FrigateViewer.apk`.

## Usage

1. Install the APK.
2. Open the app and enter your Frigate address, e.g. `http://192.168.1.10:5000`
   or `https://your-frigate:8971`.
3. The Frigate UI loads. Tap the floating gear (bottom-left) for settings.

## License

MIT. The Frigate bird mark is from the Frigate project.
