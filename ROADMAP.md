# Roadmap

## Product Direction

App Shots is a cross-platform, agent-neutral CLI with Linux as the first backend. The binary stays `appshots`; platform support is added behind backend modules while preserving the JSON contract.

## Current: Linux And Windows MVP

- Active-window capture on GNOME/X11 with automatic desktop environment discovery for TTY/SSH/agent processes.
- Windows active-window and selected-window capture through Win32 metadata plus `PrintWindow`, with .NET screen capture for virtual-screen captures and fallback cases.
- Windows best-effort text extraction through UI Automation.
- Raw `shot.png`, polished randomized `shot-card.png`, `metadata.json`, and optional `text.txt`.
- Stable JSON output for agent subprocess use.
- Codex app-server payload generation through existing `localImage` input.
- Capture history helpers: `gallery`, `latest`, and `preview`/`open`.
- Self-contained HTML gallery export through `gallery --html` for sharing batches.
- Optional stdio MCP server (`appshots mcp`) wrapping the CLI contract.

## Next: Windows Hardening

- Improve active-window capture when a window is partially covered or minimized.
- Add Windows integration tests for interactive-session capture.
- Add signed release binaries once the publishing path is stable.

## Distro And Media Test Matrix

- Add small container-based package smoke tests for major Linux distro families:
  Debian/Ubuntu, Fedora/RHEL, Arch, openSUSE, and Alpine where practical.
- Keep container tests focused on build, packaging, CLI contract, `schema`, `doctor`,
  and helper-detection behavior. Real screenshot capture still needs a graphical
  desktop session, so it belongs in VM or desktop-session tests rather than plain
  headless containers.
- Add optional VM/desktop smoke targets for GNOME X11, GNOME Wayland, KDE, and
  wlroots compositors.
- Explore shared capture artifacts for downstream video/demo workflows, including
  consistent image sizing, card style seeds, and future video format handoff points.

## Release Packaging

- Linux release archives are packaged by `scripts/package-release.sh`.
- Windows release archives are packaged by `scripts/package-release.ps1`.
- Tagged GitHub releases build and upload Linux and Windows artifacts through `.github/workflows/release.yml`.

## Later

- Wayland compositor-specific active-window support where safe and possible.
- Additional presentation styles and user-configurable style presets.
