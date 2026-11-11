#!/usr/bin/env python3
"""Turn a `/createaddition info` line into shell assignments.

    Connector at 11,100,0: mode pull, buffer 0/1000 FE, links [(15,100,0)/0/gold], pool 0/0, demand 0, t=1234
    Modular Accumulator at 10,100,0: 95000/2000000 FE (t=1234)

Prints `key=value` lines for `eval`; `ok=1` when the line parsed. Values are
plain integers or bare words, so the output is safe to `eval` in bash.
"""
from __future__ import annotations

import re
import sys

CONNECTOR = re.compile(
    r"^(Connector|Large Connector) at (-?\d+),(-?\d+),(-?\d+): mode (\w+), "
    r"buffer (\d+)/(\d+) FE, links \[(.*?)\], pool (\d+)/(\d+), demand (\d+), t=(\d+)$"
)
ACCUMULATOR = re.compile(
    r"^Modular Accumulator at (-?\d+),(-?\d+),(-?\d+): (\d+)/(\d+) FE \(t=(\d+)\)$"
)


TAG = re.compile(r" \[#\w+\]$")


def main() -> int:
    # A tagged reply ends with ` [#tag]`; the tag is the harness's, not the
    # block's.
    line = TAG.sub("", sys.argv[1].strip())
    match = CONNECTOR.match(line)
    if match:
        kind, x, y, z, mode, buffer, capacity, links, ready, next_, demand, ticks = match.groups()
        count = 0 if links.strip() == "" else links.count("(")
        values = {
            "kind": "connector",
            "large": 1 if kind == "Large Connector" else 0,
            "x": x,
            "y": y,
            "z": z,
            "mode": mode,
            "energy": buffer,
            "capacity": capacity,
            "links": count,
            "pool_ready": ready,
            "pool_next": next_,
            "demand": demand,
            "t": ticks,
        }
    else:
        match = ACCUMULATOR.match(line)
        if not match:
            print("ok=0")
            return 0
        x, y, z, energy, capacity, ticks = match.groups()
        values = {
            "kind": "accumulator",
            "large": 0,
            "x": x,
            "y": y,
            "z": z,
            "mode": "none",
            "energy": energy,
            "capacity": capacity,
            "links": 0,
            "pool_ready": 0,
            "pool_next": 0,
            "demand": 0,
            "t": ticks,
        }
    for name, value in values.items():
        print(f"{name}={value}")
    print("ok=1")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
