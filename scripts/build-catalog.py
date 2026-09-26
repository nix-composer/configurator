#!/usr/bin/env python3
"""Builds the app catalog: every installable nixpkgs package with its real
attribute, name and description, plus categories and icons for the app
store and the shell layer. Run by `nix build .#catalog` (nix/catalog);
see HANDOFF.md, "The catalog".

Inputs:
  --packages   nixpkgs' search index (pkgs/top-level/packages-info.nix)
  --appstream  AppStream data for nixpkgs (snowfallorg/nixos-appstream-data's
               appstream/ directory): names, summaries, descriptions,
               freedesktop categories, icons and screenshots of GUI apps
  --papirus    the Papirus icon theme's scalable app icons, for curated
               apps AppStream doesn't cover
  --data       this repo's data/ (app-categories.json, apps-curated.json)
  --out        writes apps.json and icons/<attr>.png there
"""

import argparse
import gzip
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import xml.etree.ElementTree as ET

LANG = "{http://www.w3.org/XML/1998/namespace}lang"

# Package sets besides the top level whose apps belong in the store.
APP_SETS = {
    "kdePackages", "pantheon", "mate", "lxqt", "xfce", "cinnamon", "lomiri",
    "jetbrains", "lumina", "budgie", "gnome", "lomiri-qt6", "dockapps",
    "sway-contrib", "wayfirePlugins",
}
# Suffixes of an app's variants: firefox-bin, vscode-fhs, obsidian-wayland.
VARIANTS = (
    "bin", "beta", "esr", "devedition", "nightly", "unstable", "stable", "fresh",
    "still", "fhs", "wayland", "wrapped", "git", "dev", "lts", "latest", "full", "qt", "gtk",
)
# Never offered: libraries and language-ecosystem packages, kernels.
LIBRARY_PATHS = (
    "pkgs/development/libraries/",
    "pkgs/development/python-modules/",
    "pkgs/development/perl-modules/",
    "pkgs/development/ruby-modules/",
    "pkgs/development/node-packages/",
    "pkgs/development/haskell-modules/",
    "pkgs/development/lua-modules/",
    "pkgs/development/ocaml-modules/",
    "pkgs/development/r-modules/",
    "pkgs/os-specific/linux/kernel/",
    "pkgs/os-specific/linux/firmware/",
)


def warn(msg):
    print(f"build-catalog: {msg}", file=sys.stderr)


def text(el, tag):
    """An element's untranslated child text."""
    for child in el.findall(tag):
        if child.get(LANG) is None and child.text:
            return " ".join(child.text.split())
    return None


def description(el):
    """AppStream's <description> markup as plain paragraphs."""
    for d in el.findall("description"):
        if d.get(LANG) is not None:
            continue
        parts = []
        for block in d:
            if block.get(LANG) is not None:
                continue
            if block.tag == "p":
                parts.append(" ".join("".join(block.itertext()).split()))
            elif block.tag in ("ul", "ol"):
                items = [
                    "• " + " ".join("".join(li.itertext()).split())
                    for li in block
                    if li.get(LANG) is None
                ]
                parts.append("\n".join(items))
        return "\n\n".join(p for p in parts if p) or None
    return None


def load_appstream(root_dir):
    """pkgname → component info, and the icon tarballs to read from."""
    comps = {}
    tarballs = []
    for origin in sorted(os.listdir(root_dir)):
        d = os.path.join(root_dir, origin)
        xml = os.path.join(d, "Components-x86_64-linux.xml.gz")
        if not os.path.exists(xml):
            continue
        for size in ("128x128", "64x64", "48x48"):
            t = os.path.join(d, f"icons-{size}.tar.gz")
            if os.path.exists(t):
                tarballs.append((size, t))
        for c in ET.parse(gzip.open(xml)).getroot():
            if c.get("type") not in ("desktop-application", "console-application"):
                continue
            pkg = c.findtext("pkgname")
            if not pkg:
                continue
            pkg = pkg.removesuffix(".out")
            icons = {
                f"{i.get('width')}x{i.get('height')}": i.text
                for i in c.findall("icon")
                if i.get("type") == "cached"
            }
            shots = []
            for s in c.iter("screenshot"):
                for img in s.findall("image"):
                    if img.get("type") == "source" and img.text:
                        # The default screenshot first.
                        if s.get("type") == "default":
                            shots.insert(0, img.text.strip())
                        else:
                            shots.append(img.text.strip())
                        break
            entry = comps.setdefault(pkg, {
                "name": text(c, "name"),
                "summary": text(c, "summary"),
                "description": description(c),
                "categories": [],
                "icons": icons,
                "screenshots": shots[:4],
                "gui": c.get("type") == "desktop-application",
                "homepage": next(
                    (u.text.strip() for u in c.findall("url") if u.get("type") == "homepage" and u.text),
                    None,
                ),
            })
            for cat in c.iter("category"):
                if cat.text and cat.text not in entry["categories"]:
                    entry["categories"].append(cat.text)
    return comps, tarballs


def store_categories(freedesktop, categories):
    """Freedesktop categories → store category ids, main one first."""
    out = []
    for cat in categories:
        if any(f in freedesktop for f in cat["freedesktop"]) and cat["id"] not in out:
            out.append(cat["id"])
    return out


def cli_categories(position, desc, categories):
    for cat in categories:
        if any(position.startswith(p + "/") for p in cat.get("paths", [])):
            return [cat["id"]]
    words = f" {desc.lower()} "
    for cat in categories:
        if any(re.search(r"\b" + re.escape(k) + r"\b", words) for k in cat.get("keywords", [])):
            return [cat["id"]]
    return []


def license_of(meta):
    lic = meta.get("license")
    lics = lic if isinstance(lic, list) else [lic] if lic else []
    names = [l.get("spdxId") or l.get("shortName") for l in lics if isinstance(l, dict)]
    return ", ".join(n for n in names if n) or None


def pretty_summary(s):
    s = " ".join((s or "").split()).rstrip(".")
    return s[:1].upper() + s[1:]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--packages", required=True)
    ap.add_argument("--appstream", required=True)
    ap.add_argument("--papirus", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--nixpkgs", default="")
    ap.add_argument("--nixpkgs-path", default="")
    args = ap.parse_args()

    packages = json.load(open(args.packages))["packages"]
    cats = json.load(open(os.path.join(args.data, "app-categories.json")))
    curated = json.load(open(os.path.join(args.data, "apps-curated.json")))
    appstream, tarballs = load_appstream(args.appstream)
    icons_dir = os.path.join(args.out, "icons")
    os.makedirs(icons_dir, exist_ok=True)

    for attr in list(curated["apps"]) + list(curated["cli"]) + curated["featured"] + curated["popularCli"]:
        if attr not in packages:
            warn(f"curated {attr} is not in nixpkgs")

    # Desktops' ecosystems: their apps and tools are store entries like
    # curated ones, with the researched category where AppStream has none.
    ecosystems = json.load(open(os.path.join(args.data, "ecosystems.json")))["ecosystems"]
    eco_apps, eco_cli = {}, {}
    for eco in ecosystems:
        for attr in eco["essentials"]:
            eco_apps.setdefault(attr, None)
        for e in eco["apps"]:
            eco_apps[e["attr"]] = eco_apps.get(e["attr"]) or e["category"]
        for e in eco["cli"]:
            eco_cli.setdefault(e["attr"], e["category"])

    entries = []
    wanted_icons = {}  # attr → cached icon file name per size
    papirus_icons = {}  # attr → Papirus icon names to try
    for attr, pkg in packages.items():
        meta = pkg.get("meta", {})
        top = attr.split(".")[0]
        if "." in attr and top not in APP_SETS:
            continue
        # Not `available`: that's false for unfree packages too, which are
        # offered (with a badge; the generator allows them).
        if meta.get("broken") or meta.get("unsupported") or meta.get("insecure"):
            continue
        position = meta.get("position") or ""
        pname = pkg.get("pname") or attr
        comp = appstream.get(attr)
        cur_app = curated["apps"].get(attr)
        cur_cli = curated["cli"].get(attr)
        if not cur_app and attr in eco_apps:
            cur_app = {"categories": [eco_apps[attr]] if eco_apps[attr] else []}
            # AppStream's categories win over the researched one.
            if comp and store_categories(comp["categories"], cats["apps"]):
                cur_app = None
            elif not cur_app["categories"]:
                cur_app = {"categories": ["utilities"]}
        elif not cur_cli and attr in eco_cli and not (comp and comp["gui"]):
            cur_cli = {"categories": [eco_cli[attr]]}
        program = meta.get("mainProgram")

        if (comp and comp["gui"]) or cur_app or position.startswith("pkgs/games/"):
            kind = "app"
        elif program or cur_cli:
            kind = "cli"
        else:
            kind = "package"
        if kind != "app" and not cur_cli:
            if position.startswith(LIBRARY_PATHS):
                continue
            if kind == "package" and ("." in attr or re.match(r"^lib[a-z0-9]", attr)):
                continue
            if attr.endswith("-unwrapped"):
                continue

        if cur_app:
            categories = list(cur_app["categories"])
        elif comp:
            categories = store_categories(comp["categories"], cats["apps"])
        elif kind == "app":
            categories = ["games"]
        elif cur_cli:
            categories = list(cur_cli["categories"])
        elif kind == "cli":
            categories = cli_categories(position, meta.get("description") or "", cats["cli"])
        else:
            categories = []

        e = {
            "attr": attr,
            # Tools and packages go by their attribute: what you'd type in
            # a shell, and unique (pnames aren't: tshark and wireshark-cli).
            "name": (cur_app or {}).get("name")
            or (cur_cli or {}).get("name")
            or (((comp or {}).get("name") or pname) if kind == "app" else attr),
            "summary": pretty_summary((comp or {}).get("summary") or meta.get("description")),
            "kind": kind,
        }
        if categories:
            e["categories"] = categories
        if meta.get("unfree"):
            e["unfree"] = True
        if pkg.get("version"):
            e["version"] = pkg["version"]
        homepage = (comp or {}).get("homepage") or meta.get("homepage")
        if isinstance(homepage, list):
            homepage = homepage[0] if homepage else None
        if homepage:
            e["homepage"] = homepage
        lic = license_of(meta)
        if lic:
            e["license"] = lic
        long = (comp or {}).get("description") or meta.get("longDescription")
        if long and kind == "app":
            e["description"] = long.strip()
        if comp and comp["screenshots"]:
            e["screenshots"] = comp["screenshots"]
        if program:
            e["program"] = program
        entries.append(e)

        if comp and comp["icons"]:
            wanted_icons[attr] = comp["icons"]
        elif kind == "app" or cur_cli:
            # Curated tools only: a tool's name often means something
            # else in an icon theme.
            cur = cur_app or cur_cli or {}
            names = [cur.get("icon"), attr.split(".")[-1], pname, program]
            papirus_icons[attr] = [n for n in names if n]

    # AppStream's cached icons, the largest size each app has.
    have = set()
    for size, tarball in tarballs:
        by_file = {}
        for attr, icons in wanted_icons.items():
            if attr not in have and size in icons:
                by_file.setdefault(icons[size], []).append(attr)
        with tarfile.open(tarball) as tar:
            for member in tar:
                attrs = by_file.get(os.path.basename(member.name))
                if not attrs or not member.isfile():
                    continue
                data = tar.extractfile(member).read()
                for attr in attrs:
                    if attr in have:
                        continue
                    with open(os.path.join(icons_dir, attr + ".png"), "wb") as f:
                        f.write(data)
                    have.add(attr)

    # Papirus for the rest of the apps, rendered to PNG; also under its
    # reverse-DNS names (neochat → org.kde.neochat).
    by_suffix = {}
    for f in sorted(os.listdir(args.papirus)):
        if f.endswith(".svg") and "." in f[:-4]:
            by_suffix.setdefault(f[:-4].rsplit(".", 1)[-1].lower(), f[:-4])
    for attr, names in papirus_icons.items():
        names = names + [by_suffix[n.lower()] for n in names if n.lower() in by_suffix]
        for name in names:
            svg = os.path.join(args.papirus, name + ".svg")
            if os.path.exists(svg):
                subprocess.run(
                    ["rsvg-convert", "-w", "128", "-h", "128", "-o",
                     os.path.join(icons_dir, attr + ".png"), svg],
                    check=True,
                )
                have.add(attr)
                break

    # Variants of an app (firefox-bin, firefox-esr-140, vscode-fhs, …) are
    # that app too: its kind, categories and icon, named after it.
    apps = {e["attr"]: e for e in entries if e["kind"] == "app"}
    variant = re.compile(r"^(.+?)[-_](" + "|".join(VARIANTS) + r")(?:[-_]?[0-9_]+)?$")
    for e in entries:
        m = variant.match(e["attr"])
        base = apps.get(m.group(1)) if m else None
        if not base or e["kind"] == "app":
            continue
        e["kind"] = "app"
        e["name"] = f"{base['name']} ({e['attr'][len(base['attr']) + 1:]})"
        if "categories" in base:
            e["categories"] = base["categories"]
        if base["attr"] in have and e["attr"] not in have:
            shutil.copy(os.path.join(icons_dir, base["attr"] + ".png"), os.path.join(icons_dir, e["attr"] + ".png"))
            have.add(e["attr"])

    for e in entries:
        if e["attr"] in have:
            e["icon"] = True

    # Every ecosystem attribute has to be in the catalog: nixpkgs renames
    # and removals show up here, not as a failed install.
    listed = {e["attr"] for e in entries}
    lost = sorted(a for a in list(eco_apps) + list(eco_cli) if a not in listed)
    if lost:
        sys.exit(f"build-catalog: data/ecosystems.json names packages that aren't installable: {', '.join(lost)}")
    entries.sort(key=lambda e: e["attr"].lower())

    catalog = {
        "version": 1,
        "nixpkgs": args.nixpkgs,
        "categories": {
            kind: [{k: c[k] for k in ("id", "name", "icon")} for c in cats[kind]]
            for kind in ("apps", "cli")
        },
        "featured": [a for a in curated["featured"] if a in packages],
        "popularCli": [a for a in curated["popularCli"] if a in packages],
        "packages": entries,
    }
    # Disk sizes (scripts/measure-sizes.py needs the network, so it's run
    # by hand and checked in), next to the catalog.
    sizes = os.path.join(args.data, "sizes.json")
    if os.path.exists(sizes):
        shutil.copy(sizes, os.path.join(args.out, "sizes.json"))
        measured = json.load(open(sizes)).get("nixpkgs", "")
        if args.nixpkgs_path and measured != os.path.basename(args.nixpkgs_path):
            warn(f"data/sizes.json was measured on {measured}; rerun scripts/measure-sizes.py")
    else:
        warn("no data/sizes.json: the installer won't know install sizes")

    with open(os.path.join(args.out, "apps.json"), "w") as f:
        json.dump(catalog, f, separators=(",", ":"), ensure_ascii=False)

    kinds = {}
    for e in entries:
        kinds[e["kind"]] = kinds.get(e["kind"], 0) + 1
    print(f"build-catalog: {len(entries)} packages {kinds}, {len(have)} icons", file=sys.stderr)


if __name__ == "__main__":
    main()
