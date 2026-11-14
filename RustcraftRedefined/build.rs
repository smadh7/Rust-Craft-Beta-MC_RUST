//! Compiles the C++ native core (`cpp/vc_core.cpp`) into one object file and links it.
//!
//! No build dependencies are used.  A C++17 compiler is located in this order:
//! `CXX_<target>` / `CXX`, then the usual drivers for the target (MSVC `cl`, `clang++`,
//! `g++`, MinGW, `zig c++`).  The core is freestanding, so any of them can build it.
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Tool {
    program: String,
    args: Vec<String>,
    msvc: bool,
    label: String,
}

fn tool(cmd: &[&str], msvc: bool) -> Tool {
    Tool {
        program: cmd[0].to_string(),
        args: cmd[1..].iter().map(|s| s.to_string()).collect(),
        msvc,
        label: cmd.join(" "),
    }
}

fn candidates(target: &str, host: &str) -> Vec<Tool> {
    let mut list = Vec::new();
    let key = format!("CXX_{}", target.replace('-', "_"));
    for var in [key.as_str(), "CXX"] {
        println!("cargo:rerun-if-env-changed={var}");
        if let Ok(value) = env::var(var) {
            let parts: Vec<&str> = value.split_whitespace().collect();
            if !parts.is_empty() {
                let stem = Path::new(parts[0]).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
                list.push(tool(&parts, stem == "cl" || stem == "clang-cl"));
            }
        }
    }
    let windows_host = host.contains("windows");
    if target.starts_with("wasm32") {
        list.push(tool(&["zig", "c++", "-target", "wasm32-freestanding"], false));
        list.push(tool(&["clang++", "--target=wasm32-unknown-unknown"], false));
        list.push(tool(&["python3", "-m", "ziglang", "c++", "-target", "wasm32-freestanding"], false));
    } else if target.contains("windows-msvc") {
        let arch = if target.starts_with("aarch64") { "aarch64" } else if target.starts_with("i686") { "x86" } else { "x86_64" };
        list.push(tool(&["cl"], true));
        list.push(tool(&["clang-cl"], true));
        let clang_target = format!("--target={target}");
        list.push(tool(&["clang++", &clang_target], false));
        let zig_target = format!("{arch}-windows-msvc");
        list.push(tool(&["zig", "c++", "-target", &zig_target], false));
        if arch == "x86_64" {
            // The core has no runtime dependencies, so a MinGW-built COFF object links fine with MSVC.
            list.push(tool(&["g++"], false));
            list.push(tool(&["x86_64-w64-mingw32-g++"], false));
        }
    } else if target.contains("windows-gnu") {
        let arch = if target.starts_with("aarch64") { "aarch64" } else if target.starts_with("i686") { "x86" } else { "x86_64" };
        if windows_host {
            list.push(tool(&["g++"], false));
        }
        let mingw = format!("{}-w64-mingw32-g++", if arch == "x86" { "i686" } else { arch });
        list.push(tool(&[&mingw], false));
        let zig_target = format!("{arch}-windows-gnu");
        list.push(tool(&["zig", "c++", "-target", &zig_target], false));
        let clang_target = format!("--target={target}");
        list.push(tool(&["clang++", &clang_target], false));
        list.push(tool(&["python3", "-m", "ziglang", "c++", "-target", &zig_target], false));
    } else if target == host {
        list.push(tool(&["c++"], false));
        list.push(tool(&["g++"], false));
        list.push(tool(&["clang++"], false));
        list.push(tool(&["zig", "c++"], false));
    } else {
        let clang_target = format!("--target={target}");
        list.push(tool(&["clang++", &clang_target], false));
        let prefixed = format!("{}-g++", target.replace("-unknown-", "-"));
        list.push(tool(&[&prefixed], false));
    }
    list
}

/// Best effort: run `cl` through vcvars when it is not on PATH (plain `cargo build` on Windows).
fn try_vcvars(source: &Path, object: &Path, target: &str) -> Result<(), String> {
    let pf = env::var("ProgramFiles(x86)").map_err(|_| "ProgramFiles(x86) is not set".to_string())?;
    let vswhere = Path::new(&pf).join("Microsoft Visual Studio").join("Installer").join("vswhere.exe");
    let out = Command::new(&vswhere)
        .args(["-latest", "-products", "*", "-requires", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64", "-property", "installationPath"])
        .output()
        .map_err(|e| format!("vswhere: {e}"))?;
    let install = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if install.is_empty() {
        return Err("vswhere found no Visual C++ tools".to_string());
    }
    let arch = if target.starts_with("aarch64") { "arm64" } else if target.starts_with("i686") { "x86" } else { "x64" };
    let vcvars = Path::new(&install).join("VC").join("Auxiliary").join("Build").join("vcvarsall.bat");
    let line = format!(
        "call \"{}\" {} >nul && cl /nologo /std:c++17 /O2 /GR- /EHs-c- /GS- /Zl /c \"{}\" /Fo\"{}\"",
        vcvars.display(),
        arch,
        source.display(),
        object.display()
    );
    let status = Command::new("cmd").args(["/C", &line]).status().map_err(|e| format!("cmd: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("vcvars + cl exited with {status}")) }
}

fn compile(t: &Tool, source: &Path, object: &Path, target: &str) -> Result<(), String> {
    let mut cmd = Command::new(&t.program);
    cmd.args(&t.args);
    if t.msvc {
        cmd.args(["/nologo", "/std:c++17", "/O2", "/GR-", "/EHs-c-", "/GS-", "/Zl", "/c"]);
        cmd.arg(source);
        cmd.arg(format!("/Fo{}", object.display()));
    } else {
        cmd.args(["-std=c++17", "-O2", "-fno-exceptions", "-fno-rtti", "-fno-threadsafe-statics", "-fno-math-errno", "-Wall", "-Wextra"]);
        if target.starts_with("wasm32") {
            cmd.args(["-ffreestanding", "-fvisibility=hidden"]);
        } else if !target.contains("windows") {
            cmd.arg("-fPIC");
        }
        cmd.arg("-c").arg(source).arg("-o").arg(object);
    }
    match cmd.output() {
        Err(e) => Err(format!("{}: {e}", t.label)),
        Ok(out) if out.status.success() => {
            for line in String::from_utf8_lossy(&out.stderr).lines().filter(|l| l.contains("warning")) {
                println!("cargo:warning={line}");
            }
            Ok(())
        }
        Ok(out) => Err(format!("{}: {}\n{}", t.label, out.status, String::from_utf8_lossy(&out.stderr))),
    }
}

fn main() {
    let target = env::var("TARGET").expect("TARGET");
    let host = env::var("HOST").expect("HOST");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let source = Path::new("cpp").join("vc_core.cpp");
    for file in ["vc_core.cpp", "vc_core.h", "vc_noise.h", "vc_terrain.cpp", "vc_mesh.cpp"] {
        println!("cargo:rerun-if-changed=cpp/{file}");
    }
    println!("cargo:rerun-if-changed=build.rs");
    let object = out_dir.join(if target.contains("msvc") { "vccore.obj" } else { "vccore.o" });

    let mut errors = Vec::new();
    let mut built = false;
    for t in candidates(&target, &host) {
        match compile(&t, &source, &object, &target) {
            Ok(()) => {
                built = true;
                break;
            }
            Err(e) => errors.push(e),
        }
    }
    if !built && target.contains("windows-msvc") && host.contains("windows") {
        match try_vcvars(&source, &object, &target) {
            Ok(()) => built = true,
            Err(e) => errors.push(e),
        }
    }
    if !built {
        panic!(
            "\n\nRustcraft Redefined needs a C++17 compiler for its native core (cpp/).\n\
             Install one of: Visual Studio Build Tools, LLVM/clang, MinGW-w64, GCC or Zig,\n\
             or point the CXX environment variable at a compiler driver.\n\nAttempts:\n  {}\n",
            errors.join("\n  ")
        );
    }
    // A single object passed straight to the linker: applies to binaries, tests and the wasm cdylib.
    println!("cargo:rustc-link-arg={}", object.display());
}
