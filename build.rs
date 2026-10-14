use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=native/ghostty_bridge.c");
    println!("cargo:rerun-if-changed=native/shutdown_bridge.m");
    println!("cargo:rerun-if-env-changed=GHOSTTY_SOURCE");
    if env::var_os("CARGO_FEATURE_NATIVE").is_none() {
        return;
    }
    let source = PathBuf::from(env::var_os("GHOSTTY_SOURCE").unwrap_or(".spike/ghostty".into()));
    assert!(
        source.join("zig-out/lib/libghostty-vt.a").exists(),
        "Build pinned Ghostty first: python3 scripts/bootstrap_ghostty.py"
    );
    cc::Build::new()
        .file("native/ghostty_bridge.c")
        .include(source.join("include"))
        .warnings(true)
        .compile("tasks_ghostty_bridge");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new().file("native/shutdown_bridge.m").compile("tasks_shutdown_bridge");
        println!("cargo:rustc-link-lib=framework=Cocoa");
    }
    println!(
        "cargo:rustc-link-search=native={}",
        source.join("zig-out/lib").canonicalize().unwrap().display()
    );
    println!("cargo:rustc-link-lib=static=ghostty-vt");
}
