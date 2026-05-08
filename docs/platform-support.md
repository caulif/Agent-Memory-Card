# Platform Support

Agent Memory Kernel is currently published for Windows first.

- Windows

## Official Packaged Target

The current public release target is the Windows desktop installer.

CLI binaries and Bun packages are intentionally not published yet. They remain
developer surfaces until the command-line workflow is more complete.

## Desktop App

The desktop workspace is built with Tauri v2. The frontend lives in `app/` and the Tauri shell lives in `src-tauri/`.

Run it during development:

```bash
bun run app:dev
```

Build release bundles:

```bash
bun run tauri:build
```

On Windows, Tauri release artifacts are written under:

```text
src-tauri/target/release/bundle/
```

The public release currently uploads the NSIS `*setup.exe` installer only.

## CI Policy

Every push and pull request runs:

- Rust format, clippy, and release builds on Windows
- Frontend production build on Windows

Release builds package one Windows desktop installer.

Tagged releases build the Windows Tauri desktop installer so users can install the app without running the development toolchain.

## Current Development Host

The current active development and manual verification host is Windows. macOS and Linux are not release targets yet.
