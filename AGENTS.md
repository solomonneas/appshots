# Repository Guidance

## Definition of Done
Before reporting any change complete, run the verification entrypoint and confirm it passes:

```
./scripts/verify
```

It runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`, in that order. These are exactly what CI runs (`.github/workflows/ci.yml`, Linux and Windows). Clippy warnings fail the build. Report actual results. If any command fails, report the failure verbatim and do not claim success. Re-run after the final edit; earlier green runs do not count.

## Project Shape
- Cloche is a Rust CLI for desktop capture: polished screenshots (Shots), planned short recordings (Reels), and stable JSON output for scripts and agent workflows. The repo directory and crate are `cloche`; the project was rebranded from App Shots, and only the `appshots` binary remains from that era.
- Two binaries share one code path: `cloche` (`src/main.rs`) is primary, `appshots` (`src/bin/appshots.rs`) is a compatibility alias. Both call `cloche::run()` from `src/lib.rs`. Keep the alias working while existing automation and MCP configs still reference it.
- Layout: `src/cli.rs` (clap commands), `src/capture/` (Linux in `mod.rs`, Windows in `windows.rs`), `src/polish.rs` (shot-card rendering), `src/mcp.rs` (stdio MCP server), `src/html.rs` (gallery export), `src/codex.rs` (turn/start payloads), `src/contract.rs` (JSON output schema).
- The MCP server shells out to `current_exe` so the CLI JSON contract stays the single source of truth. Do not refactor MCP to call internal functions directly.
- Making a non-obvious tradeoff -> log it in `implementation-notes.md` in the same change.
- Tempted to add a dependency -> don't. Hand-roll small helpers instead (the base64 encoder in `html.rs` is the precedent). If a crate is unavoidable, use `default-features = false` and justify it.
- Touching packaging -> verify with `bash scripts/package-release.sh` (builds a release archive into gitignored `dist/`).
- Adding a top-level file that must ship with the crate -> add it to the `include` list in `Cargo.toml`, or `cargo publish` silently drops it.

## Hard Prohibitions
- A test or clippy lint fails -> fix the code. Never delete, weaken, `#[ignore]`, `#[allow]`, or otherwise skip a failing test or lint to get green. If the check itself seems wrong, stop and ask.
- Unsure what a command, flag, or API does -> read the code first (`src/`, `scripts/`, `Cargo.toml`). Never invent commands, flags, or API facts.
- Blocked by sandboxing, auth, a missing tool, or no display -> report the exact blocker verbatim and stop. Do not work around it or fabricate the result.
- Never `git push` unless the user explicitly asks, and never pass `--no-verify` to bypass any git hook on commit or push.
- Never commit `memory/` or `.brigade/`, and never weaken their gitignore rules. They are local-only.

## Safety Rules
- Capture commands interact with the live desktop session -> do not run a real capture unless the user explicitly asks in this session. Tests must not require a display.
- `scripts/proxmox-vm-smoke.sh` creates and destroys VMs on a Proxmox host. It is dry-run by default. Do not pass `--apply` or `--destroy` unless the user explicitly asks in this session.
- `scripts/windows-live-capture-test.ps1` and `scripts/windows-schedule-live-test.ps1` run live capture on a Windows machine. Do not run them unless the user explicitly asks in this session.

## Gotchas
- Capture exits 0 only when a raw image was written. Text extraction and card rendering failures are warnings by design -> do not promote them to errors.
- Running from SSH or an agent process -> desktop env vars (DISPLAY, XAUTHORITY, DBUS_SESSION_BUS_ADDRESS) are discovered from running desktop processes. GNOME/KDE Wayland may block silent active-window capture; `--target screen` is the fallback.
- Windows capture uses `PrintWindow` with a .NET `CopyFromScreen` fallback and requires an interactive desktop session. Plain OpenSSH sessions can build and run `doctor` but cannot capture.
- Writing a card-rendering test -> styling is randomized; pass `--style-seed <number>` to make output deterministic.
- Adding unit tests -> they live in `#[cfg(test)]` modules inside `src/*.rs`. There is no `tests/` dir; do not create one for unit tests.

## Memory Handoff
At the end of any substantial task, write a handoff note to `.claude/memory-handoffs/` using that directory's `TEMPLATE.md`. Record durable discoveries, gotchas, and decisions. Do not wait to be reminded.

Note: `.claude/` is deliberately gitignored, so the handoff directory and its template exist only on the maintainer's machines and are absent from fresh clones. If you are working from a clone without it, skip the handoff; it is a maintainer-local memory flow, not a contributor requirement.
