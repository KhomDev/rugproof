#!/usr/bin/env python3
"""Regenerate the derived exercise files from their reference versions.

    python3 exercises/scripts/sync_generated.py          # rewrite files
    python3 exercises/scripts/sync_generated.py --check  # CI: fail if stale

* `0N-*/starter/src/lib.rs` (N = 1..4) comes from `solutions/src/lib.rs`:
  every `pub fn` whose doc comment starts with `/// <level>.<n>` gets its
  body replaced by `todo!("exercise: <name>")`. Guide 00's starter is
  hand-written and left alone.
* `0N-*/capstone/src/lib.rs` (N = 0..4) comes from
  `capstone-solution/src/lib.rs`: every block from a `// FIX` line to its
  `// END FIX` line is removed, `// WAS: <code>` comments become code
  again, and a trailing `#[cfg(test)] mod tests` is dropped.

The generated file keeps its own leading `#![...]` / `//!` header, so edit
headers in the generated files and everything else in the reference ones.
Run `cargo fmt` in the workspace afterwards.
"""
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent


def split_header(text):
    lines = text.split("\n")
    i = 0
    while i < len(lines) and (lines[i].startswith("//!") or lines[i].startswith("#![")):
        i += 1
    return lines[:i], lines[i:]


def make_starter(solution):
    _, lines = split_header(solution)
    out, doc_is_exercise, j = [], False, 0
    while j < len(lines):
        line, s = lines[j], lines[j].strip()
        if s.startswith("///"):
            if not out or not out[-1].strip().startswith("///"):
                doc_is_exercise = bool(re.match(r"/// \d\.\d", s))
            out.append(line)
            j += 1
            continue
        if s.startswith("#[") and doc_is_exercise:
            out.append(line)
            j += 1
            continue
        m = re.match(r"\s*pub fn (\w+)", line)
        if m and doc_is_exercise:
            sig = []
            while True:
                sig.append(lines[j])
                if lines[j].rstrip().endswith("{"):
                    break
                j += 1
            out.extend(sig)
            depth = sum(l.count("{") - l.count("}") for l in sig)
            j += 1
            while depth > 0:
                depth += lines[j].count("{") - lines[j].count("}")
                j += 1
            indent = re.match(r"\s*", sig[0]).group(0)
            out.append(f'{indent}    todo!("exercise: {m.group(1)}")')
            out.append(f"{indent}}}")
            doc_is_exercise = False
            continue
        if s and not s.startswith("#["):
            doc_is_exercise = False
        out.append(line)
        j += 1
    return out


def make_vulnerable(solution):
    _, lines = split_header(solution)
    out, skip = [], False
    for l in lines:
        if l.startswith("#[cfg(test)]"):
            break
        if "// END FIX" in l:
            skip = False
            continue
        if "// FIX" in l:
            skip = True
            continue
        if skip:
            continue
        st = l.lstrip()
        if st.startswith("// WAS: "):
            l = l[: len(l) - len(st)] + st[len("// WAS: "):]
        out.append(l)
    while out and out[-1].strip() == "":
        out.pop()
    out.append("")
    return out


def main():
    check = "--check" in sys.argv
    stale = []
    jobs = []
    for ws in sorted(ROOT.glob("0[0-9]-*")):
        if not ws.name.startswith("00"):
            jobs.append((ws / "solutions/src/lib.rs", ws / "starter/src/lib.rs", make_starter))
        jobs.append((ws / "capstone-solution/src/lib.rs", ws / "capstone/src/lib.rs", make_vulnerable))
    for src, dst, fn in jobs:
        header, _ = split_header(dst.read_text())
        new = "\n".join(header + fn(src.read_text()))
        # format exactly like `cargo fmt` would, so --check is stable
        new = subprocess.run(["rustfmt", "--edition", "2021", "--emit", "stdout", "-q"],
                             input=new, capture_output=True, text=True, check=True).stdout
        if dst.read_text() != new:
            stale.append(str(dst.relative_to(ROOT)))
            if not check:
                dst.write_text(new)
    if check and stale:
        print("stale generated files (run exercises/scripts/sync_generated.py):")
        print("\n".join("  " + s for s in stale))
        sys.exit(1)
    print("updated: " + (", ".join(stale) if stale else "nothing"))


if __name__ == "__main__":
    main()
