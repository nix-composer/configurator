#!/usr/bin/env bash
# Generates a host for every installable desktop with its whole ecosystem
# ticked (and once more with a launch and a command shortcut) and evaluates
# each system fully, as a fresh install would (newest nixos-26.05). Needs the
# network. Shortcut variants of desktops whose binds are read-only are
# refused by the generator, as the installer doesn't offer them.
#   scripts/eval-ecosystems.sh [jobs]
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
bin=$(nix build --no-link --print-out-paths "$repo#configurator")/bin
python3 - "$repo" "$work" <<'PY'
import json, sys, os
repo, work = sys.argv[1:]
base = json.load(open(f"{repo}/examples/answers/gnome.json"))
base.pop("keybinds", None); base.pop("loginManager", None)
eco = {e["desktop"]: e for e in json.load(open(f"{repo}/data/ecosystems.json"))["ecosystems"]}
name = lambda x: x if isinstance(x, str) else (x.get("attr") or x.get("id"))
os.makedirs(f"{work}/answers")
for d in json.load(open(f"{repo}/data/desktops.json"))["desktops"]:
    if d.get("unavailable"):
        continue
    e = eco.get(d["id"], {})
    a = json.loads(json.dumps(base))
    a["desktop"] = {"id": d["id"], "ecosystem": True}
    a["apps"] = {"packages": sorted({name(x) for x in e.get("apps", [])} | {"firefox"})}
    a["shell"] = dict(a.get("shell") or {}, packages=sorted({name(x) for x in e.get("cli", [])}))
    json.dump(a, open(f"{work}/answers/{d['id']}.json", "w"))
    a["keybinds"] = {"SUPER + E": {"launch": "firefox"}, "SUPER + H": {"exec": "notify-send hello"}}
    json.dump(a, open(f"{work}/answers/{d['id']}+keys.json", "w"))
PY
one() {
  local name=$1 out="$work/hosts/$1"
  if ! msg=$("$bin/configurator" generate --answers "$work/answers/$name.json" --out "$out" 2>&1); then
    case $msg in *"not supported yet"*) echo "skip  $name (${msg##*: })" ;; *) echo "FAIL  $name: generate: ${msg##*$'\n'}" ;; esac
    return
  fi
  host=$(grep -o 'nixosConfigurations\.[A-Za-z0-9_-]*' "$out/flake.nix" | head -1 | cut -d. -f2)
  (cd "$out" && git init -q && git add -A)
  if msg=$(nix eval --raw "$out#nixosConfigurations.$host.config.system.build.toplevel.drvPath" 2>&1); then
    echo "ok    $name"
  else
    echo "FAIL  $name: $(grep -E '^ *error:' <<<"$msg" | tail -1)"
  fi
}
export -f one; export work bin
ls "$work/answers" | sed 's/\.json$//' | xargs -P "${1:-6}" -I{} bash -c 'one {}' | sort | tee "$work/results"
! grep -q '^FAIL' "$work/results"
