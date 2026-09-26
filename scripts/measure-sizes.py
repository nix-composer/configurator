#!/usr/bin/env python3
"""Measures how much disk what the installer offers takes, into
data/sizes.json: every GUI app, the curated and ecosystem tools, the AI
agents, and each desktop's base system, as closures (the store paths each
needs) over one table of path sizes. The installer adds up the union of
what's picked, so shared libraries (Qt, GTK, KDE's frameworks) count once.

Asks the binary cache (nothing is downloaded or built), so it needs the
network and isn't part of the sandboxed catalog build; rerun it when
nixpkgs moves:

    nix build .#catalog -o result-catalog
    scripts/measure-sizes.py            # writes data/sizes.json

Packages the cache doesn't have (unfree ones it can't redistribute) are
left out; the installer says their size is unknown.
"""

import json
import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CACHE = "https://cache.nixos.org"
KIB = 1024


def nixpkgs():
    return subprocess.run(
        ["nix", "eval", "--raw", "--impure", "--expr",
         f'(builtins.getFlake "git+file://{ROOT}").inputs.nixpkgs.outPath'],
        check=True, capture_output=True, text=True,
    ).stdout.strip()


def wanted(catalog_dir):
    """The attributes the installer offers by name."""
    catalog = json.load(open(os.path.join(catalog_dir, "apps.json")))
    data = lambda f: json.load(open(os.path.join(ROOT, "data", f)))
    attrs = {p["attr"] for p in catalog["packages"] if p["kind"] == "app"}
    attrs |= set(catalog["popularCli"])
    attrs |= set(data("apps-curated.json")["cli"])
    for eco in data("ecosystems.json")["ecosystems"]:
        attrs |= {p["attr"] for p in eco["apps"] + eco["cli"]}
    attrs |= {a["attr"] for a in data("agents.json")["agents"]}
    for p in data("profiles.json")["profiles"]:
        attrs |= set(p["apps"])
    return sorted(attrs)


def path_infos(paths):
    """Store path → (narSize, references) for the closures of `paths`,
    from the cache; paths it lacks are skipped."""
    infos = {}

    def query(batch):
        r = subprocess.run(
            ["nix", "path-info", "--store", CACHE, "--recursive", "--json", *batch],
            capture_output=True, text=True,
        )
        if r.returncode == 0:
            for path, info in json.loads(r.stdout).items():
                if info:
                    infos[path] = (info["narSize"], info.get("references", []))
        elif len(batch) > 1:
            half = len(batch) // 2
            query(batch[:half])
            query(batch[half:])
        else:
            print(f"measure-sizes: not in the cache: {batch[0]}", file=sys.stderr)

    todo = sorted(set(paths))
    for i in range(0, len(todo), 200):
        query(todo[i:i + 200])
        print(f"measure-sizes: {min(i + 200, len(todo))}/{len(todo)} roots", file=sys.stderr)
    return infos


def closure(root, infos, memo):
    if root in memo:
        return memo[root]
    seen, stack = set(), [root]
    while stack:
        p = stack.pop()
        if p in seen or p not in infos:
            continue
        seen.add(p)
        stack.extend(infos[p][1])
    memo[root] = seen
    return seen


def main():
    catalog_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "result-catalog")
    attrs = wanted(catalog_dir)
    np = nixpkgs()
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump(attrs, f)
    evaluated = json.loads(subprocess.run(
        ["nix", "eval", "--json", "--impure", "--expr",
         f"import {ROOT}/scripts/sizes.nix {{ nixpkgs = {np}; attrs = {f.name}; "
         f"registry = {ROOT}/data/desktops.json; }}"],
        check=True, capture_output=True, text=True,
    ).stdout)
    os.unlink(f.name)

    roots = [p for p in evaluated["packages"].values() if p]
    for paths in evaluated["systems"].values():
        roots += paths
    infos = path_infos(roots)

    # One table of paths (sizes in KiB), closures as indexes into it.
    memo, table, index = {}, [], {}

    def ids(paths):
        out = []
        for p in sorted(paths):
            if p not in index:
                index[p] = len(table)
                table.append(max(1, round(infos[p][0] / KIB)))
            out.append(index[p])
        return sorted(out)

    packages, unknown = {}, []
    for attr, out in sorted(evaluated["packages"].items()):
        if out and out in infos:
            packages[attr] = ids(closure(out, infos, memo))
        else:
            unknown.append(attr)
    systems = {}
    for desktop, paths in sorted(evaluated["systems"].items()):
        union = set()
        for p in paths:
            union |= closure(p, infos, memo)
        systems[desktop] = ids(union)

    rev = os.path.basename(np)
    sizes = {
        "$comment": "Written by scripts/measure-sizes.py: disk use (KiB) of store paths, and the closures (indexes into `paths`) of the packages the installer offers and of each desktop's base system (`none`: no desktop). Rerun when nixpkgs moves.",
        "nixpkgs": rev,
        "paths": table,
        "packages": packages,
        "systems": systems,
        "unknown": unknown,
    }
    out = os.path.join(ROOT, "data", "sizes.json")
    with open(out, "w") as f:
        # Compact, one closure per line, so updates diff by package.
        f.write("{\n")
        f.write(f'  "$comment": {json.dumps(sizes["$comment"])},\n')
        f.write(f'  "nixpkgs": {json.dumps(rev)},\n')
        f.write(f'  "paths": {json.dumps(table, separators=(",", ":"))},\n')
        for key in ("packages", "systems"):
            f.write(f'  "{key}": {{\n')
            items = list(sizes[key].items())
            for i, (k, v) in enumerate(items):
                comma = "," if i + 1 < len(items) else ""
                f.write(f'    {json.dumps(k)}: {json.dumps(v, separators=(",", ":"))}{comma}\n')
            f.write("  },\n")
        f.write(f'  "unknown": {json.dumps(unknown)}\n}}\n')
    total = sum(table) * KIB / 1e9
    print(f"measure-sizes: {len(packages)} packages, {len(systems)} systems, {len(table)} paths "
          f"({total:.1f} GB), {len(unknown)} unknown; wrote {out} "
          f"({os.path.getsize(out) / 1e6:.1f} MB)", file=sys.stderr)


if __name__ == "__main__":
    main()
