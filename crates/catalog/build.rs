//! Builds the web app and AI agent icons (data/webapps/<id>.png,
//! data/agents/<id>.png) into the crate.

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

fn main() {
    let mut out = String::from("/// Icons by id (PNG).\n");
    table(&mut out, "WEBAPP_ICONS", "webapps");
    table(&mut out, "AGENT_ICONS", "agents");
    let dest = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("icons.rs");
    std::fs::write(dest, out).unwrap();
}
