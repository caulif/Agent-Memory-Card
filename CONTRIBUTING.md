# Contributing

Thanks for considering a contribution to Agent Memory Kernel.

## Development Setup

Install Rust and Bun, then run:

```bash
bun install
bun install --cwd app
cargo build
bun run --cwd app build
```

Run the desktop workspace during development:

```bash
bun run app:dev
```

## Verification

Before opening a pull request, run the checks that match your change:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo build --release
cargo build --manifest-path src-tauri/Cargo.toml --release
bun run --cwd app build
```

For UI changes, also run the Tauri app locally and verify the affected flow in
the desktop shell.

## Pull Requests

- Keep changes focused and explain the user-visible behavior.
- Keep local tests outside the public repository.
- Do not commit local runtime data such as `.agent-kernel/`, `.claude/skills/`,
  `.codex/`, `.superpowers/`, build outputs, or generated caches.
- Redact private conversation content, credentials, local paths, and API keys
  from issues and screenshots.
