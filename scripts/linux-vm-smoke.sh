#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Run appshots smoke checks over SSH against a Linux VM.

Usage:
  scripts/linux-vm-smoke.sh user@host [options]

Options:
  --artifact-url URL      Release tarball to test.
  --ssh-option OPTION    Extra ssh option, repeatable.
  -h, --help             Show this help.

The smoke checks do not require a GUI. In a headless VM, doctor is expected to
return a non-zero status while still producing JSON diagnostics.
USAGE
}

target=""
artifact_url="https://github.com/solomonneas/appshots/releases/download/v0.1.0/appshots-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
ssh_options=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --artifact-url) artifact_url="$2"; shift 2 ;;
    --ssh-option) ssh_options+=("$2"); shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *)
      if [[ -z "$target" ]]; then
        target="$1"
        shift
      else
        echo "unexpected argument: $1" >&2
        usage >&2
        exit 2
      fi
      ;;
  esac
done

if [[ -z "$target" ]]; then
  usage >&2
  exit 2
fi

remote_script=$(cat <<'REMOTE'
set -euo pipefail

workdir="$(mktemp -d)"
cleanup() {
  rm -rf "$workdir"
}
trap cleanup EXIT

cd "$workdir"

need() {
  command -v "$1" >/dev/null || {
    echo "missing required command in VM: $1" >&2
    exit 1
  }
}

need curl
need tar

curl -fL "$ARTIFACT_URL" -o appshots.tar.gz
tar -xzf appshots.tar.gz
bin="$(find . -type f -name appshots -perm -111 | head -n 1)"
test -n "$bin"

"$bin" --help >/tmp/appshots-help.txt
"$bin" schema --compact >/tmp/appshots-schema.json

set +e
"$bin" doctor --format json >/tmp/appshots-doctor.json
doctor_status=$?
set -e

test -s /tmp/appshots-help.txt
test -s /tmp/appshots-schema.json
test -s /tmp/appshots-doctor.json

case "$doctor_status" in
  0|1) ;;
  *)
    echo "unexpected doctor exit status: $doctor_status" >&2
    exit 1
    ;;
esac

printf 'appshots smoke ok\n'
printf 'doctor exit status: %s\n' "$doctor_status"
head -20 /tmp/appshots-doctor.json
REMOTE
)

ssh "${ssh_options[@]}" "$target" "ARTIFACT_URL=$(printf '%q' "$artifact_url") bash -s" <<<"$remote_script"
