#!/usr/bin/env bash
#
# Regenerates the terminal screenshots in docs/images/ from real command output.
#
#   ./docs/tools/capture.sh
#
# Commands run under a pseudo-terminal so the runtime enables colour exactly as
# it would for a human, and the raw output is rendered to SVG by
# render_terminal.py. Nothing here is hand-written, so the screenshots stay
# honest as the runtime changes.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

images="docs/images"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

mkdir -p "$images"

if [[ ! -x target/release/chaqmoq ]]; then
  echo "building the release binary first..."
  cargo build --release --quiet
fi
bin="$root/target/release/chaqmoq"

# Runs a command attached to a PTY so colour detection behaves as it would for a
# human. See pty_run.py for why this is not `script(1)`.
run_in_pty() {
  python3 docs/tools/pty_run.py "$@" </dev/null
}

# Same, but forwards stdin to the child (used for the REPL).
run_in_pty_stdin() {
  python3 docs/tools/pty_run.py "$@"
}

render() {
  local name="$1" title="$2"
  python3 docs/tools/render_terminal.py \
    --title "$title" --out "$images/$name.svg" <"$work/$name.txt"
}

# One command, its output, and the command itself in the title bar.
shot() {
  local name="$1" title="$2"
  shift 2
  echo "capturing $name"
  run_in_pty "$@" >"$work/$name.txt" 2>&1 || true
  render "$name" "$title"
}

# Several commands in sequence, each preceded by a shell prompt line.
session() {
  local name="$1" title="$2"
  shift 2
  echo "capturing $name"
  : >"$work/$name.txt"
  for args in "$@"; do
    # \033[34m is blue: the prompt, so it reads as a shell transcript.
    printf '\033[34m$\033[0m chaqmoq %s\n' "$args" >>"$work/$name.txt"
    # shellcheck disable=SC2086 -- word splitting is what turns the string into argv.
    run_in_pty "$bin" $args >>"$work/$name.txt" 2>&1 || true
    printf '\n' >>"$work/$name.txt"
  done
  render "$name" "$title"
}

shot hello \
  "chaqmoq run examples/hello.ts" \
  "$bin" run examples/hello.ts

shot inspector \
  "chaqmoq run examples/showcase.ts" \
  "$bin" run examples/showcase.ts

shot errors \
  "chaqmoq run broken.ts" \
  "$bin" run docs/snippets/broken.ts

shot server \
  "chaqmoq run --allow-net server.ts" \
  "$bin" run --allow-net docs/snippets/server-demo.ts

shot tests \
  "chaqmoq test examples/tests/" \
  "$bin" test examples/tests/

shot info \
  "chaqmoq info" \
  "$bin" info

shot help \
  "chaqmoq --help" \
  "$bin" --help

session permissions \
  "the permission model" \
  "run docs/snippets/permission-demo.ts" \
  "run --allow-read=/etc docs/snippets/permission-demo.ts"

# The REPL reads from stdin, so its session is piped in.
echo "capturing repl"
printf '%s\n' \
  'const runtime = { name: "chaqmoq", v8: Chaqmoq.version.v8 }' \
  'runtime' \
  '[1, 2, 3].map((n) => n ** 3)' \
  'await Promise.resolve("top-level await works")' \
  'new Map([["uz", "Salom"], ["en", "Hello"]])' \
  '.exit' \
  | run_in_pty_stdin "$bin" repl >"$work/repl.txt" 2>&1 || true
render repl "chaqmoq repl"

echo "done -- screenshots are in $images/"
