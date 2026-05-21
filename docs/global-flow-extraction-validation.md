# Global Flow Extraction Validation

This checklist applies when changing extraction quality code, especially:

- `src/observation/conversation.rs`
- `src/observation/chunked.rs`
- `src/observation/flow.rs`
- `src/extract.rs`
- `src/extract/candidate_factory.rs`
- `src/extract/gate.rs`
- `src/extract/quality_gate.rs`
- `src/extract/memory_gate.rs`

## Required Checks

```powershell
cargo fmt --check
cargo test --lib
cargo test --test extract_quality_v2
cargo run --quiet -- eval --golden-set --project $PWD
cargo run --quiet -- observe replay --project $PWD --home $env:USERPROFILE --target codex --target claude-code --engine local --dry-run
cargo run --quiet -- observe replay --project "C:\QianCi" --home $env:USERPROFILE --target codex --target claude-code --engine local --dry-run
```

## Review Rule

Read the final candidate preview text from replay output. A change is not complete if it only improves counts while the generated cards are vague, unsupported, duplicated, or current-task chatter.

## Privacy Boundary

Real local histories and Obsidian notes are only validation inputs. Do not commit them, upload them to GitHub, paste raw excerpts into issues, or add them to Golden Set. Convert any discovered miss into a synthetic or fully anonymized test.
