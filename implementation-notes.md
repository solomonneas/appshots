# Implementation Notes

Running log of decisions and tradeoffs not captured in commit messages or the spec.

## Repo hygiene (2026-06-01)

- Added `.github/workflows/ci.yml` running fmt/clippy/test on Linux + Windows for
  pushes to `master` and all PRs. Clippy runs with `-D warnings`; verified the tree
  is already warning-clean so CI starts green. Mirrors `release.yml` conventions
  (`actions/checkout@v4`, `dtolnay/rust-toolchain@stable`).
- Added `LICENSE` (Apache-2.0) to match the `license` field already declared in
  `Cargo.toml`. Copyright holder: Solomon Neas, 2026.

## Gallery HTML export (2026-06-01)

- New `src/html.rs`: pure, dependency-free helpers (`base64_encode`, `escape_html`,
  `render`) plus a `GalleryItem` view struct. Kept it a leaf module so `cli.rs`
  depends on `html`, not the reverse.
- Decision: embed images as base64 `data:` URIs so the exported file is a single
  self-contained, shareable artifact (matches the roadmap "sharing batches" goal).
  Tradeoff: larger HTML files vs. zero external file dependencies. Self-containment
  wins for a share-and-send use case.
- Decision: hand-rolled base64 encoder instead of pulling the `base64` crate. The
  repo keeps deps minimal (all `default-features = false`); the encoder is ~20 lines
  with RFC 4648 test vectors, so a new dependency was not worth it.
- `gallery --html <PATH>` writes the file and still prints the normal gallery JSON,
  now with an added `htmlPath` field. `--open` opens the result via the existing
  `open_path` helper. Image selection prefers `presentation_image`, falls back to the
  raw `image`, matching `preview` semantics.

## MCP wrapper (2026-06-01)

- New `src/mcp.rs`: a minimal stdio JSON-RPC 2.0 server (`cloche mcp`) implementing
  `initialize`, `ping`, `tools/list`, `tools/call`, and notification handling.
- Decision: tool calls shell out to the Cloche binary itself (`current_exe`) rather
  than calling internal functions. This keeps the capture contract in exactly one
  place, guarantees byte-identical output to the CLI, and matches the roadmap phrasing
  ("MCP wrapper around the CLI contract"). Tradeoff: one subprocess per tool call;
  negligible next to the cost of a screen capture.
- Dispatch is split into a pure `dispatch()` function (testable with a mock tool
  runner) and `run_tool_via_subprocess()` (the side-effecting part). `tool_command_args`
  is also pure and unit-tested.
- Protocol version reported: `2025-06-18`.

## Polish command (2026-06-11)

- New `cloche polish <INPUT>` styles an existing image into the presentation card
  (rounded window, layered shadows, gradient backdrop) without a live capture.
  Motivation: agent sessions kept being asked to "clean up this screenshot with
  Cloche", which previously had no real implementation, so agents improvised
  ImageMagick recreations with flat white or transparent canvases.
- Output is a single PNG (default `<input>-card.png` next to the input). `--out`
  must end in `.png`; the card always carries alpha for its rounded canvas
  corners, so silently re-encoding to JPEG would corrupt the look.
- `--palette <name>` pins one of the named palettes while every other style
  parameter still derives from the seed (`style_with_palette` in `polish.rs`).
  Palette names come from the `PALETTES` table via `palette_names()` so clap
  help, MCP schema enum, and validation share one source of truth.
- No `metadata.json` and no frame-extent cropping: polish is a single-file
  transform, not a capture. `gallery`/`latest` only scan capture dirs and that
  stays true.
- Result contract is a new `PolishResult` (camelCase wire keys) rather than
  reusing `AppshotResult`, which is capture-shaped (target, backend, window).

## Dependency feature trim (2026-06-12)

- All deps now declare `default-features = false` with explicit feature lists,
  per the AGENTS.md rule. Net effect: clap drops `color`/`suggestions`
  (anstream, colorchoice, strsim gone from the tree; anstyle remains because
  clap_builder depends on it unconditionally). Help and error output verified
  readable without them.
- `image` gained `jpeg` and `webp` decode features: `polish` documents JPEG and
  WebP inputs, and png-only silently broke that contract (caught by a failing
  test before the fix). Encode surface is still PNG-only by design.
- `rand` features: `std`, `std_rng`, `thread_rng`, `os_rng` are exactly what
  `rand::rng()` + `StdRng::seed_from_u64` need. `schemars` keeps `derive` and
  `chrono` (DateTime fields in the contract derive JsonSchema).
