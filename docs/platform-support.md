# Platform Support

Agent Memory Kernel is currently published for Windows first.

- Windows

## Official Packaged Targets

The Bun package looks for these prebuilt binaries first:

| OS | Architecture | Package path |
| --- | --- | --- |
| Windows | x64 | `bin/win32-x64/agent-kernel.exe` |

If no packaged binary exists for the current platform, the wrapper falls back to local Cargo outputs:

1. `target/release/agent-kernel[.exe]`
2. `target/debug/agent-kernel[.exe]`
3. development fallback through `cargo build`

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

Depending on the installed Tauri bundler toolchain, this directory can contain installer formats such as MSI or NSIS EXE bundles.

## CI Policy

Every push and pull request runs:

- Rust format, clippy, and release builds on Windows
- Frontend production build on Windows

Release builds package the Windows CLI binary and Windows Tauri desktop bundle.

Tagged releases also build the Windows Tauri desktop bundle so users can install the app without running the development toolchain.

## Current Development Host

The current active development and manual verification host is Windows. macOS and Linux are not release targets yet.
