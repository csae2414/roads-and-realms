#!/usr/bin/env python3
"""The long functions clippy lets through (`#[allow(clippy::too_many_lines)]`) may not grow.

Architect round 3, #10: the 150-line lint only blocked new long functions; the exempt ones grew
by 30-100 lines each. tools/fnlen.txt keeps each one's length; a longer one, or a new exemption,
fails. Shorter is fine: `tools/fnlen.py --update` writes the new lengths.
usage: tools/fnlen.py [--update]
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BASE = ROOT / "tools" / "fnlen.txt"
FN = re.compile(r"^(\s*)(?:pub(?:\([a-z]+\))? )?fn ([a-z0-9_]+)")


def exempt():
    """path::name -> lines, for every exempt function (rustfmt: it ends at its indent's `}`)."""
    found = {}
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        lines = path.read_text().split("\n")
        for i, line in enumerate(lines):
            if "clippy::too_many_lines" not in line:
                continue
            j = i + 1
            while not FN.match(lines[j]):
                j += 1
            indent, name = FN.match(lines[j]).groups()
            end = j
            while lines[end] != indent + "}":
                end += 1
            found[f"{path.relative_to(ROOT)}::{name}"] = end - j + 1
    return found


def main():
    now = exempt()
    if "--update" in sys.argv:
        BASE.write_text("".join(f"{k} {v}\n" for k, v in sorted(now.items())))
        return 0
    base = {}
    for line in BASE.read_text().splitlines():
        k, v = line.rsplit(" ", 1)
        base[k] = int(v)
    bad = []
    for k, v in sorted(now.items()):
        if k not in base:
            bad.append(f"{k}: newly exempt ({v} lines); split it instead")
        elif v > base[k]:
            bad.append(f"{k}: {base[k]} -> {v} lines; exempt functions may not grow")
    for b in bad:
        print("fnlen:", b)
    shrunk = [k for k, v in now.items() if k in base and v < base[k]]
    if shrunk and not bad:
        print("fnlen: shorter now (run tools/fnlen.py --update):", ", ".join(shrunk))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
