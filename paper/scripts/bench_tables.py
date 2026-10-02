#!/usr/bin/env python3
"""Generate the evaluation tables of the paper from encore-benchmarks.

Usage: bench_tables.py <path to encore-benchmarks/results/benchmarks.jsonl>

Writes LaTeX table bodies to stdout. For each (workload, variant, board, N,
profile, RAM budget, optimizer) the latest row wins, as in the benchmark
repository's own `cargo xtask check`. Main board: qemu-an505 (Cortex-M33);
the summary at the end compares it with qemu-lm3s6965 (Cortex-M3).
"""
import json
import sys

BOARD = "qemu-an505"
WORKLOADS = [
    ("w1_apdu", "W1 APDU + BER-TLV"),
    ("w2_rlp", "W2 RLP decoder"),
    ("w3_policy", "W3 BIP32 policy"),
    ("w4_pin", "W4 PIN state machine"),
    ("w5_update", "W5 A/B update"),
    ("w6_cobs", "W6 COBS framing"),
    ("w7_store", "W7 credential store"),
    ("w8_crc", "W8 CRC-16/32"),
]
# `cargo xtask minheap` results, from each workload's README (KiB, whole case
# list, Cortex-M3). W4's README does not report one.
MIN_HEAP_KIB = {"w1_apdu": 10.5, "w2_rlp": 16.75, "w3_policy": 1, "w4_pin": None,
                "w5_update": 28, "w6_cobs": 32.25, "w7_store": 12.75, "w8_crc": 0.5}


def load(path):
    latest = {}
    for line in open(path):
        if line.strip():
            r = json.loads(line)
            b = r["build"]
            latest[(r["workload"], r["variant"], r["board"], r["n"], r["profile"],
                    b.get("ram_kb"), b.get("cps_optimize"))] = r
    return latest


def get(latest, w, v, n, board=BOARD, profile="timing"):
    for k, r in latest.items():
        if k[:5] == (w, v, board, n, profile) and k[5] == 50 and k[6] in (None, True):
            return r
    return None


def ns(latest, w, v, board=BOARD):
    return sorted({k[3] for k in latest if k[0] == w and k[1] == v and k[2] == board})


def insns(r):
    return r["insns"]["median"]


def kib(b):
    return f"{b / 1024:.1f}"


def main(path):
    L = load(path)
    print("% Table: cost at the largest N that both E and C complete")
    for w, name in WORKLOADS:
        ok_c = [n for n in ns(L, w, "C") if get(L, w, "C", n)["ok"]]
        n = max(ok_c) if ok_c else max(ns(L, w, "E"))
        R, E, C = get(L, w, "R", n), get(L, w, "E", n), get(L, w, "C", n)
        prog = E["build"]["program_bytes"]
        cr = f"{insns(C) / insns(R):.0f}" if C else "--"
        ec = f"{insns(E) / insns(C):.1f}" if C else "--"
        fc = kib(C["size"]["flash_bytes"]) if C else "--"
        print(f"{name} & {n} & {insns(R):,} & {insns(E) / insns(R):.0f} & {cr} & {ec} & "
              f"{kib(R['size']['flash_bytes'])} & {fc} & {kib(E['size']['flash_bytes'])} & "
              f"{kib(prog)} \\\\".replace(",", "{,}"))

    print("\n% Table: feasibility and GC at the largest N")
    for w, name in WORKLOADS:
        e_ns, c_ns = ns(L, w, "E"), ns(L, w, "C")
        e_ok = [n for n in e_ns if get(L, w, "E", n)["ok"]]
        c_ok = [n for n in c_ns if get(L, w, "C", n)["ok"]]
        nmax = max(e_ns)
        M = get(L, w, "E", nmax, profile="memory")
        gc = M["gc"]
        mh = MIN_HEAP_KIB[w]
        c_txt = (f"{max(c_ok)}" if c_ok else "--") if c_ns else "not built"
        print(f"{name} & {min(e_ns)}--{nmax} & {max(e_ok)} & {c_txt} & "
              f"{mh if mh is not None else '--'} & {gc['count']} & {gc['pct']:.0f} & "
              f"{gc['pause_max_since_boot'] / 1000:.0f} \\\\")

    print("\n% Summary")
    er, cr, ec, opt, over = [], [], [], [], []
    for w, _ in WORKLOADS:
        for n in ns(L, w, "C"):
            R, E, C = get(L, w, "R", n), get(L, w, "E", n), get(L, w, "C", n)
            if C and C["ok"]:
                ec.append(insns(E) / insns(C)); cr.append(insns(C) / insns(R))
        for n in ns(L, w, "E"):
            er.append(insns(get(L, w, "E", n)) / insns(get(L, w, "R", n)))
        R0, E0 = get(L, w, "R", ns(L, w, "R")[0]), get(L, w, "E", ns(L, w, "E")[0])
        over.append(E0["size"]["flash_bytes"] - R0["size"]["flash_bytes"] - E0["build"]["program_bytes"])
        for k, r in L.items():
            if (k[0] == w and k[1] == "E" and k[2] == "qemu-lm3s6965" and k[4] == "timing"
                    and k[6] is False and r["ok"] and r.get("insns")):
                on = next(x for kk, x in L.items() if kk[:5] == k[:5] and kk[5] == k[5] and kk[6] is True)
                opt.append(insns(r) / insns(on))
    rng = lambda xs, f="{:.1f}": f"{f.format(min(xs))}--{f.format(max(xs))}"
    print(f"% E/C instructions: {rng(ec)}   C/R: {rng(cr, '{:.0f}')}   E/R: {rng(er, '{:.0f}')}")
    print(f"% optimizer off/on (M3): {rng(opt)}   E runtime flash overhead over R: {rng(over, '{:.0f}')} B")
    worst = 0
    for k, r in L.items():
        if k[1] in "EC" and k[2] == "qemu-lm3s6965" and k[4] == "timing" and k[5] == 50 and k[6] in (None, True) and r["ok"]:
            other = get(L, k[0], k[1], k[3])
            if other and other["ok"]:
                worst = max(worst, abs(insns(r) / insns(other) - 1))
    print(f"% largest M3 vs M33 instruction-count difference: {worst * 100:.1f}%")


if __name__ == "__main__":
    main(sys.argv[1])
