# Platform Support

Agent-Kernel is a Rust binary distributed through a Bun wrapper. The goal is to support the three mainstream desktop operating systems:

- Windows
- macOS
- Linux

## Official Packaged Targets

The Bun package looks for these prebuilt binaries first:

| OS | Architecture | Package path |
| --- | --- | --- |
| Windows | x64 | `bin/win32-x64/agent-kernel.exe` |
| Linux | x64 | `bin/linux-x64/agent-kernel` |
| macOS | x64 | `bin/darwin-x64/agent-kernel` |
| macOS | arm64 | `bin/darwin-arm64/agent-kernel` |

If no packaged binary exists for the current platform, the wrapper falls back to local Cargo outputs:

1. `target/release/agent-kernel[.exe]`
2. `target/debug/agent-kernel[.exe]`
3. development fallback through `cargo build`

## Native App

`agent-kernel app` uses Rust `eframe/egui`, so it is a compiled native desktop UI rather than a WebView, browser page, or localhost web app.

Linux CI installs the native desktop build dependencies needed by the egui/winit stack:

```bash
sudo apt-get install -y libxkbcommon-dev libwayland-dev libx11-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

## CI Policy

Every push and pull request runs:

- Rust format, clippy, and tests on Windows, Linux, and macOS
- Bun wrapper tests on Windows, Linux, and macOS

Release builds package the four official binary targets listed above.

## Current Development Host

The current active development and manual verification host is Windows. macOS and Linux support is enforced through GitHub Actions and release packaging.
