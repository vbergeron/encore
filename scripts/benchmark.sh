#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RESULTS="$ROOT/benchmarks.jsonl"
EXAMPLE="$ROOT/examples/sort"
TARGET="thumbv7m-none-eabi"
ELF="$EXAMPLE/target/$TARGET/release/encore-baremetal-sort"

usage() {
    echo "Usage: $0 <name>" >&2
    echo "  name: label for this benchmark run (e.g. 'baseline', 'remove-bounds-checks')" >&2
    exit 1
}

[ $# -ge 1 ] || usage
NAME="$1"

echo "=== Building sort example (release) ==="
(cd "$EXAMPLE" && cargo build --release)

echo "=== Running under QEMU with tracing ==="
TRACEFILE=$(mktemp)
trap 'rm -f "$TRACEFILE"' EXIT

PROGRAM_OUTPUT=$(qemu-system-arm \
    -machine lm3s6965evb \
    -semihosting-config enable=on \
    -nographic \
    -icount shift=0,align=off \
    -d in_asm,exec,nochain \
    -D "$TRACEFILE" \
    -kernel "$ELF" 2>&1)

echo "=== Counting instructions ==="
INSN_COUNT=$(python3 "$SCRIPT_DIR/count_insns.py" < "$TRACEFILE")
TIMESTAMP=$(date -Iseconds)
GIT_REV=$(git -C "$ROOT" rev-parse --short HEAD)

echo "$PROGRAM_OUTPUT"
echo "---"
echo "insn_count: $INSN_COUNT"

printf '{"name":"%s","timestamp":"%s","git":"%s","insn_count":%s,"output":"%s"}\n' \
    "$NAME" "$TIMESTAMP" "$GIT_REV" "$INSN_COUNT" \
    "$(echo "$PROGRAM_OUTPUT" | tr '\n' ' ' | sed 's/"/\\"/g')" \
    >> "$RESULTS"

echo "=== Result appended to $RESULTS ==="
