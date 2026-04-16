#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RESULTS="$ROOT/benchmarks.jsonl"
EXAMPLES_DIR="$ROOT/examples"
TARGET="thumbv7m-none-eabi"

usage() {
    echo "Usage: $0 <name>" >&2
    echo "  name: label for this benchmark run (e.g. 'baseline', 'remove-bounds-checks')" >&2
    exit 1
}

[ $# -ge 1 ] || usage
NAME="$1"

TIMESTAMP=$(date -Iseconds)
GIT_REV=$(git -C "$ROOT" rev-parse --short HEAD)

run_one() {
    local example="$1"
    local frontend="$2"
    local features_flag="$3"
    local pkg_name="$4"
    local elf="$EXAMPLES_DIR/$example/target/$TARGET/release/$pkg_name"

    echo "--- $example ($frontend) ---"

    if ! (cd "$EXAMPLES_DIR/$example" && cargo build --release $features_flag 2>&1); then
        echo "  SKIP (build failed)"
        return 0
    fi

    local tracefile
    tracefile=$(mktemp)

    local output
    output=$(qemu-system-arm \
        -machine lm3s6965evb \
        -semihosting-config enable=on \
        -nographic \
        -icount shift=0,align=off \
        -d in_asm,exec,nochain \
        -D "$tracefile" \
        -kernel "$elf" 2>&1) || true

    local insn_count
    insn_count=$(python3 "$SCRIPT_DIR/count_insns.py" < "$tracefile")
    rm -f "$tracefile"

    local safe_output
    safe_output=$(echo "$output" | tr '\n' ' ' | sed 's/"/\\"/g')

    echo "  output: $output"
    echo "  insn_count: $insn_count"

    printf '{"name":"%s","example":"%s","frontend":"%s","timestamp":"%s","git":"%s","insn_count":%s,"output":"%s"}\n' \
        "$NAME" "$example" "$frontend" "$TIMESTAMP" "$GIT_REV" "$insn_count" "$safe_output" \
        >> "$RESULTS"
}

for example_dir in "$EXAMPLES_DIR"/*/; do
    example=$(basename "$example_dir")
    pkg_name=$(grep '^name' "$example_dir/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')

    if [ -f "$example_dir/$example.fleche" ]; then
        run_one "$example" "fleche" "--no-default-features --features fleche" "$pkg_name"
    fi

    if [ -f "$example_dir/$example.scm" ]; then
        run_one "$example" "scheme" "--no-default-features --features scheme" "$pkg_name"
    fi
done

echo ""
echo "=== All results appended to $RESULTS ==="
