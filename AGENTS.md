# AGENTS.md

Personal Linux desktop app that edits a marked block of `/etc/hosts`. The design is settled and implemented in layers: `src/block.rs` (parser/serializer), `src/hosts.rs` + `src/helper.rs` (write path), `src/import.rs`, `src/backup.rs`, and the Iced layer in `src/app.rs` with the layout in `src/app/view.rs`.

## Read first

- `docs/design.md` — agreed behavior. Implement that; the MVP scope and non-goals are explicit.
- `docs/adr/0001`–`0006` — why the model is what it is (structured editing, pkexec helper, Iced, managed block, block format, in-app helper install).
- `GLOSSARY.md` — domain vocabulary. Docs are Portuguese; every UI string is English.
- `README.md` — user-facing overview; keep it in sync when behavior changes.

## Dependencies

Use the `cargo` CLI for all package management: `cargo add`, `cargo remove`, `cargo update`, `cargo tree`, `cargo metadata`. Never hand-edit `[dependencies]` in `Cargo.toml`; only bypass cargo when it genuinely cannot perform the operation.

Iced 0.14 is the chosen UI stack (ADR-0003) and is pinned in `Cargo.toml`. Edition 2024, Linux only.

## Commands

```sh
cargo run --release                 # dev run; helper must be installed once
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

## Safety invariants (do not relax)

- The GUI never writes `/etc/hosts`. Writes go through the root-owned helper `/usr/local/libexec/rust-hosts-helper` via `pkexec`; the GUI stays unprivileged and refuses to start as root.
- Only the `# BEGIN rust-hosts` / `# END rust-hosts` block is editable; everything outside it is preserved byte for byte.
- Apply verifies the loaded file hash, writes a timestamped backup to `~/.local/state/rust-hosts/backups/` (last 20), preserves owner/mode, and is atomic (temp + fsync + rename).
- Never test against the real `/etc/hosts` and never invoke `pkexec` from tests; use fixtures.

## Layout notes

- Keep the parser and helper logic UI-free and unit-testable; Iced (`update`/`view`) should only call into them.
- `.agents/skills/` is generated and pinned by `skills-lock.json`; do not edit those files by hand.
