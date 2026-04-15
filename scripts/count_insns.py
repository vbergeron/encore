#!/usr/bin/env python3
"""
Parse a QEMU trace file produced with `-d in_asm,exec,nochain` and compute
the total number of guest Thumb instructions executed.

Each `IN:` block defines a Translation Block (TB) with its raw bytes (OBJD-T).
Each `Trace` line records a TB execution, identified by host address.
We map host addresses to instruction counts, then sum over all executions.
"""

import sys
import re

TRACE_RE = re.compile(r"^Trace \d+: (0x[0-9a-f]+) ")

def count_thumb_insns(objd_hex: str) -> int:
    raw = bytes.fromhex(objd_hex)
    n = 0
    i = 0
    while i < len(raw):
        if i + 1 >= len(raw):
            break
        hw = raw[i] | (raw[i + 1] << 8)
        if (hw & 0xE000) == 0xE000 and (hw & 0x1800) != 0:
            i += 4  # 32-bit Thumb2
        else:
            i += 2  # 16-bit Thumb
        n += 1
    return n

def main():
    tb_insn_count: dict[str, int] = {}
    pending_objd: str | None = None
    total_insns = 0
    total_tbs = 0

    for line in sys.stdin:
        if line.startswith("OBJD-T: "):
            pending_objd = line[8:].strip()
            continue

        m = TRACE_RE.match(line)
        if m:
            total_tbs += 1
            addr = m.group(1)
            if addr not in tb_insn_count:
                if pending_objd is not None:
                    tb_insn_count[addr] = count_thumb_insns(pending_objd)
                else:
                    tb_insn_count[addr] = 0
            pending_objd = None
            total_insns += tb_insn_count[addr]

    print(total_insns)

if __name__ == "__main__":
    main()
