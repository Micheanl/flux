use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/simd.zig");
    println!("cargo:rerun-if-env-changed=ZIG");
    println!("cargo:rustc-check-cfg=cfg(zig_native)");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is required"));
    let zig = env::var("ZIG").unwrap_or_else(|_| "zig".to_string());

    let Ok(version) = Command::new(&zig).arg("version").output() else {
        println!("cargo:warning=zig was not found; building simd with the rust fallback");
        return;
    };
    if !version.status.success() {
        println!("cargo:warning=zig could not be executed; building simd with the rust fallback");
        return;
    }

    let mut build = Command::new(&zig);
    build.args(["build-lib", "src/simd.zig", "-O", "ReleaseFast", "-fstrip"]);

    let host = env::var("HOST").unwrap_or_default();
    let target = env::var("TARGET").unwrap_or_default();
    if host != target {
        let Some(zig_target) = zig_target(&target) else {
            println!(
                "cargo:warning=unsupported target {target}; building simd with the rust fallback"
            );
            return;
        };
        build.args(["-target", zig_target]);
    }
    if !target.contains("windows") && !target.contains("wasm") {
        build.arg("-fPIC");
    }

    let archive = out_dir.join("libsimd.a");
    build.arg(format!("-femit-bin={}", archive.display()));

    let ok = matches!(build.status(), Ok(status) if status.success()) && archive.exists();
    if !ok {
        println!("cargo:warning=zig build failed; building simd with the rust fallback");
        return;
    }

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=simd");
    println!("cargo:rustc-cfg=zig_native");
}

fn zig_target(rust_target: &str) -> Option<&'static str> {
    Some(match rust_target {
        "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu",
        "x86_64-unknown-linux-musl" => "x86_64-linux-musl",
        "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu",
        "aarch64-unknown-linux-musl" => "aarch64-linux-musl",
        "x86_64-pc-windows-msvc" => "x86_64-windows-msvc",
        "x86_64-pc-windows-gnu" => "x86_64-windows-gnu",
        "aarch64-pc-windows-msvc" => "aarch64-windows-msvc",
        "aarch64-apple-darwin" => "aarch64-macos",
        "x86_64-apple-darwin" => "x86_64-macos",
        _ => return None,
    })
}
