fn main() {
    // the built-in plugins and the SDK are embedded with include_dir!, which doesn't track changes by itself
    println!("cargo:rerun-if-changed=../../plugins");
    println!("cargo:rerun-if-changed=../../sdk/python");
}
