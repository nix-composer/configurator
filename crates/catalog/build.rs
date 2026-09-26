//! Builds the icons of web apps, AI agents, dev templates and containers
//! (data/{webapps,agents,dev-templates,containers}/<id>.png) into the crate,
//! with the desktops' default keybinds and flake desktops' own catalogs.

use std::fmt::Write;

fn table(out: &mut String, name: &str, dir: &str) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .join(dir);
    println!("cargo::rerun-if-changed={}", dir.display());
    let mut icons: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| e.ok()?.path().canonicalize().ok())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    icons.sort();
    writeln!(out, "static {name}: &[(&str, &[u8])] = &[").unwrap();
    for path in icons {
        let id = path.file_stem().unwrap().to_str().unwrap();
        writeln!(
            out,
            "    ({id:?}, include_bytes!({:?})),",
            path.to_str().unwrap()
        )
        .unwrap();
    }
    out.push_str("];\n");
}

/// `data/keybinds/<desktop>.json`, the desktops' default keybinds.
fn keybinds(out: &mut String) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/keybinds");
    println!("cargo::rerun-if-changed={}", dir.display());
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| e.ok()?.path().canonicalize().ok())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    out.push_str("/// Default keybinds by desktop id (JSON).\nconst DEFAULT_KEYBINDS: &[(&str, &str)] = &[\n");
    for path in files {
        let id = path.file_stem().unwrap().to_str().unwrap();
        writeln!(
            out,
            "    ({id:?}, include_str!({:?})),",
            path.to_str().unwrap()
        )
        .unwrap();
    }
    out.push_str("];\n");
}

/// Flake desktops' catalogs, `<name>.json` in `$CONFIGURATOR_DESKTOP_CATALOGS`
/// (nix/desktop-catalogs.nix; the dev shell and the packages set it).
fn desktop_catalogs(out: &mut String) {
    println!("cargo::rerun-if-env-changed=CONFIGURATOR_DESKTOP_CATALOGS");
    let mut files = Vec::new();
    match std::env::var_os("CONFIGURATOR_DESKTOP_CATALOGS") {
        Some(dir) => {
            let dir = std::path::PathBuf::from(dir);
            println!("cargo::rerun-if-changed={}", dir.display());
            files = std::fs::read_dir(&dir)
                .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
                .filter_map(|e| e.ok()?.path().canonicalize().ok())
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .collect();
            files.sort();
        }
        None => println!(
            "cargo::warning=CONFIGURATOR_DESKTOP_CATALOGS isn't set (use `nix develop`): \
             flake desktops' own picks (Omarchy's apps, web apps, tools) are left out"
        ),
    }
    out.push_str("/// Flake desktops' catalogs by name (JSON).\nconst DESKTOP_CATALOGS: &[(&str, &str)] = &[\n");
    for path in files {
        let name = path.file_stem().unwrap().to_str().unwrap();
        writeln!(
            out,
            "    ({name:?}, include_str!({:?})),",
            path.to_str().unwrap()
        )
        .unwrap();
    }
    out.push_str("];\n");
}

fn main() {
    let mut out = String::from("/// Icons by id (PNG).\n");
    table(&mut out, "WEBAPP_ICONS", "webapps");
    table(&mut out, "AGENT_ICONS", "agents");
    table(&mut out, "DEV_TEMPLATE_ICONS", "dev-templates");
    table(&mut out, "CONTAINER_ICONS", "containers");
    keybinds(&mut out);
    desktop_catalogs(&mut out);
    let dest = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("icons.rs");
    std::fs::write(dest, out).unwrap();
}
