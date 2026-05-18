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

## Release Smoke Checks

The release workflow now verifies:

- Tauri desktop tests pass on Windows before packaging.
- A Windows desktop executable exists after the release build.
- Exactly one NSIS `*setup.exe` installer is created.
- The installer artifact and a `release-known-limitations.md` note are uploaded together.

CI also smoke checks the CLI release binary with `agent-kernel.exe --version` and verifies the frontend production bundle contains `app/dist/index.html` plus assets.

## Known Limitations

- Windows is the only packaged release target.
- CLI binaries and Bun packages are developer surfaces, not public release artifacts.
- Provider calls require a configured API key, reachable Base URL, and proxy environment when the local network needs one.
- Generated Agent artifacts remain explicit, reviewable file writes; automatic background sync is not enabled.

## Current Development Host

The current active development and manual verification host is Windows. macOS and Linux are not release targets yet.
