# App Shots

`appshots` is an agent-neutral app screenshot capture CLI. It captures the active app/window, writes a PNG plus metadata, and prints stable JSON so any agent can call it as a subprocess.

Linux and Windows are supported so agents can depend on one command and one output contract across desktops.

OpenAI's Appshots behavior is the compatibility target:

- Capture the frontmost window only.
- Include an image of the visible window.
- Include available text from that window when the OS/app exposes it, possibly including text outside the visible scroll area.
- Treat the result like an attachment to the agent thread.

Reference: <https://developers.openai.com/codex/appshots>

The Codex repository already accepts local images through app-server v2 `turn/start` input:

```json
{ "type": "localImage", "path": "/absolute/path/to/shot.png", "detail": "high" }
```

This tool uses that existing local-image path instead of adding a runtime dependency on any one agent. Upstream Codex currently documents Appshot creation as a macOS app feature; the CLI can resume threads containing Appshots but cannot create new Appshots itself.

## Install

```bash
cargo install --path .
```

Or:

```bash
bash scripts/install.sh
```

On Windows:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/install.ps1
```

## Release Packaging

Build a local release archive:

```bash
bash scripts/package-release.sh
```

On Windows:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package-release.ps1
```

Archives are written under `dist/`. Tagged GitHub releases are packaged by `.github/workflows/release.yml` for Linux and Windows.

## Commands

```bash
appshots doctor --format json
appshots list-windows --format json
appshots capture --target active --presentation both --out-dir /tmp/appshot --format json
appshots capture --target active --style-seed 12345 --out-dir /tmp/appshot --format json
appshots capture --target screen --out-dir /tmp/appshot --format json
appshots capture --target window --title Firefox --out-dir /tmp/appshot --format json
appshots gallery --limit 10
appshots latest
appshots preview
appshots open /tmp/appshot
appshots schema
appshots codex-payload --thread-id THREAD_ID /tmp/appshot
```

## Output Files

Each successful capture directory contains:

- `shot.png`, the raw captured image.
- `shot-card.png`, a Codex-style presentation image with background cleanup, rounded corners, padding, and a soft shadow.
- `metadata.json`, the same JSON object printed to stdout.
- `text.txt`, optional best-effort accessible text from the focused app.

Capture exits with `0` only when a raw image was written. Text extraction and presentation-image failures are warnings because accessibility support and desktop compositing vary by toolkit, app, desktop environment, and OS. `--target screen` exists as a fallback/debug mode, but `--target active` is the Appshots-compatible default.

Use `--presentation raw`, `--presentation card`, or `--presentation both` to control output image generation. Use `--style-seed <number>` to reproduce a randomized card style exactly.

## Agent Use

Any shell-capable agent can call:

```bash
appshots capture --target active --out-dir /tmp/appshot-$(date +%s) --format json
```

Then parse `image.path` from stdout or read `metadata.json`.

Codex app-server clients can turn a capture into a ready `turn/start` payload:

```bash
appshots codex-payload --thread-id "$THREAD_ID" /tmp/appshot-123
```

Claude Code, OpenClaw, Hermes, and other agents should treat `appshots` as a normal subprocess tool. The core command has no MCP or desktop-app dependency.

## Linux Backend Notes

- X11 active/window capture uses `xdotool`/`wmctrl` for window metadata and ImageMagick `import` for PNG capture.
- Wayland wlroots screen capture uses `grim`.
- GNOME/KDE Wayland may block silent active-window capture by design. Use `--target screen` or run `appshots doctor --format json` for diagnostics.
- Text extraction is best-effort through AT-SPI using Python GI when available.

If you are invoking `appshots` from SSH, a TTY, or an agent process that did not inherit the desktop environment, `appshots` will try to discover the live desktop variables from desktop processes. On GNOME X11 they usually look like:

```bash
export DISPLAY=:1
export XAUTHORITY=/run/user/$(id -u)/gdm/Xauthority
export DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u)/bus
export XDG_SESSION_TYPE=x11
```

You can discover the active values from a desktop process:

```bash
tr '\0' '\n' </proc/$(pgrep -u "$(id -u)" -n gnome-shell)/environ | grep -E '^(DISPLAY|XAUTHORITY|DBUS_SESSION_BUS_ADDRESS|XDG_SESSION_TYPE)='
```

## Windows Backend Notes

- Active/window capture uses Win32 foreground-window and top-level-window metadata, then captures the visible window bounds through .NET `CopyFromScreen`.
- Screen capture uses the Windows virtual screen.
- Text extraction is best-effort through UI Automation.
- Capture must run in a logged-in interactive desktop session. Plain OpenSSH sessions can build and run `doctor`, but Windows blocks screen capture from the non-interactive SSH service session.

## Roadmap

See [ROADMAP.md](ROADMAP.md).
