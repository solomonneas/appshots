# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0] - 2026-06-12

### Added
- `polish` command and matching MCP tool: style any existing image (PNG, JPEG,
  or WebP) into the presentation card without a live capture, with `--palette`,
  `--style-seed`, and `--out` controls.
- `schema --for polish` exposes the polish JSON contract alongside the capture
  contract.
- MSRV (1.85) check job in CI.
- Unit coverage for the Codex `turn/start` payload contract and the text
  persistence path.

### Changed
- Rebranded from App Shots to Cloche: `cloche` is the primary binary and crate;
  `appshots` remains as a compatibility alias.
- Presentation cards redesigned with vibrant 3-stop gradients, glow spots,
  light streaks, grain, and rounded canvas corners.
- All dependencies now build with `default-features = false` and explicit
  feature lists; clap's color and suggestion machinery dropped from the tree.

### Fixed
- `polish` decodes JPEG and WebP inputs as documented; previously only PNG
  decoding was compiled in.

## [0.1.0] - 2026-06-02

### Added
- Initial release as App Shots: active/window/screen capture on Linux (X11)
  and Windows, raw `shot.png` plus presentation `shot-card.png`, stable JSON
  output with `metadata.json`, best-effort text extraction, `gallery`/`latest`/
  `preview` helpers, HTML gallery export, Codex `turn/start` payload
  generation, and a stdio MCP server.

[Unreleased]: https://github.com/escoffier-labs/cloche/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/escoffier-labs/cloche/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/escoffier-labs/cloche/releases/tag/v0.1.0
