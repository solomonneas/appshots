# Roadmap

## Product Direction

Cloche is an open-source, OS-agnostic desktop capture tool for agents, scripts, and human workflows. The primary command is `cloche`; the existing `appshots` command remains as a compatibility alias while the project transitions from App Shots to Cloche.

Cloche has two first-class modes:

- **Shots**: still screenshots, available now.
- **Reels**: short screen recordings, planned from the existing Appreels prototype.

GIF support is planned as an export target after Reels, not as the primary recording backend.

## Current: Shots MVP

- Linux active-window capture on GNOME/X11 with automatic desktop environment discovery for TTY/SSH/agent processes.
- Windows active-window and selected-window capture through Win32 metadata plus `PrintWindow`, with .NET screen capture for virtual-screen captures and fallback cases.
- Windows best-effort text extraction through UI Automation.
- Raw `shot.png`, polished randomized `shot-card.png`, `metadata.json`, and optional `text.txt`.
- `polish` command and MCP tool that style any existing image into the same presentation card, so agents and scripts can reframe screenshots they did not capture with Cloche.
- Stable JSON output for agent subprocess use.
- Codex app-server payload generation through existing `localImage` input.
- Capture history helpers: `gallery`, `latest`, and `preview`/`open`.
- Self-contained HTML gallery export through `gallery --html` for sharing batches.
- Optional stdio MCP server (`cloche mcp`) wrapping the CLI contract.
- Compatibility binary and MCP path through `appshots`.

## Near Term: Rename And Repository Transition

- Keep `cloche` as the primary binary/package name.
- Keep `appshots` as a compatibility binary until existing automation, docs, release assets, and downstream MCP configs have moved.
- Move the GitHub repository to Escoffier Labs after explicit transfer approval.
- Update release names, badges, install scripts, package archives, and smoke scripts to prefer Cloche.
- Keep the old Appshots context in docs only where it explains compatibility or Codex's documented Appshots feature.

## Next: Reels Mode

Reels should merge the useful Appreels work into Cloche without making video feel bolted on.

- Add a `cloche reels` command group once the integration starts.
- Bring over Appreels capture and render pieces behind Cloche naming:
  - `record` for raw short desktop captures.
  - `render` for polished video framing, captions, cursor emphasis, title cards, and outro cards.
  - `perform-terminal` and `perform-browser` once the scripting path is stable enough.
- Share presentation styling between Shots and Reels so both modes look like one product.
- Preserve stable JSON contracts with `ok`, `warnings`, `errors`, paths, durations, and generated artifact metadata.
- Keep X11/Linux as the first Reels backend because the current Appreels prototype already works there.
- Treat Windows and macOS Reels as later backend work unless a user need forces them earlier.

## Reels Integration Sequence

1. Create the Cloche command shape:
   - `cloche shots capture`
   - `cloche shots gallery`
   - `cloche reels record`
   - `cloche reels render`
   - keep top-level `cloche capture` as a Shots shortcut for compatibility.
2. Extract or vendor shared presentation code from Appreels so Shots and Reels use the same palette, padding, corner radius, and shadow model.
3. Port the Appreels script schema under Cloche naming and keep the JSON schema command.
4. Add Reels output metadata:
   - `rawVideo`
   - `reel`
   - `cursorTrack`
   - `cueFile`
   - `durationMs`
   - `presentationStyle`
5. Add MCP tools for Reels only after the CLI contract is stable.
6. Add release packaging for any required video assets, helper docs, and platform dependency checks.

## GIF Export

GIF export is intentionally later.

- Add `cloche reels export-gif --input demo.mp4 --out demo.gif`.
- Prefer generating GIFs from finished Reels so captions, cursor emphasis, zooms, and framing stay consistent.
- Add size controls before shipping:
  - width/height limits
  - fps
  - palette generation
  - max duration
  - optional loop count
- Keep MP4/WebM as the quality defaults and GIF as a sharing fallback.

## Windows Hardening

- Improve active-window capture when a window is partially covered or minimized.
- Add Windows integration tests for interactive-session capture.
- Add signed release binaries once the publishing path is stable.

## Distro And Media Test Matrix

- Add small container-based package smoke tests for major Linux distro families:
  Debian/Ubuntu, Fedora/RHEL, Arch, openSUSE, and Alpine where practical.
- Keep container tests focused on build, packaging, CLI contract, `schema`, `doctor`, and helper-detection behavior.
- Keep real screenshot capture in VM or desktop-session tests because it needs a graphical desktop session.
- Add optional VM/desktop smoke targets for GNOME X11, GNOME Wayland, KDE, and wlroots compositors.
- Add Reels media smoke tests once video mode lands.

## Release Packaging

- Linux release archives are packaged by `scripts/package-release.sh`.
- Windows release archives are packaged by `scripts/package-release.ps1`.
- Tagged GitHub releases build and upload Linux and Windows artifacts through `.github/workflows/release.yml`.

## Later

- Wayland compositor-specific active-window support where safe and possible.
- Additional presentation styles and user-configurable style presets.
- macOS backend exploration after Linux and Windows are boring.
