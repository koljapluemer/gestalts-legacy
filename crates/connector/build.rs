//! Copies what the connector needs of the built-in plugins (manifests, gestalts, workers; not their UI or decks,
//! and never `node_modules`) to `$OUT_DIR/plugins`, which `include_dir!` embeds.

use std::fs;
use std::path::Path;

fn main() {
    // include_dir! doesn't track changes by itself
    println!("cargo:rerun-if-changed=../../plugins");
    println!("cargo:rerun-if-changed=../../sdk/python");
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("plugins");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).unwrap();
    for entry in fs::read_dir("../../plugins").unwrap().flatten() {
        let src = entry.path();
        if !src.join("plugin.yml").is_file() {
            continue;
        }
        let dest = out.join(entry.file_name());
        fs::create_dir_all(&dest).unwrap();
        fs::copy(src.join("plugin.yml"), dest.join("plugin.yml")).unwrap();
        for sub in ["gestalts", "worker"] {
            if src.join(sub).is_dir() {
                copy_tree(&src.join(sub), &dest.join(sub));
            }
        }
    }
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let name = entry.file_name();
        let name_s = name.to_string_lossy();
        if name_s.starts_with('.') || name_s == "__pycache__" || name_s == "node_modules" {
            continue;
        }
        let ty = entry.file_type().unwrap();
        if ty.is_dir() {
            copy_tree(&entry.path(), &to.join(&name));
        } else if ty.is_file() {
            fs::copy(entry.path(), to.join(&name)).unwrap();
        }
    }
}
