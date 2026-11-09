use std::{env, path::PathBuf, process::Command};
fn run(command: &mut Command) {
    let status = command.status().expect("could not start C++ compiler or archiver");
    assert!(status.success(), "native build command failed: {command:?}");
}
fn main() {
    let windows = env::var("TARGET").unwrap().contains("windows");
    let (cc, ar) = if windows { ("x86_64-w64-mingw32-g++-posix", "x86_64-w64-mingw32-ar") } else { ("g++", "ar") };
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let obj = out.join("vccore.o");
    run(Command::new(cc).args(["-O3", "-ffast-math", "-fno-exceptions", "-fno-rtti", "-DNDEBUG", "-std=c++17", "-c", "cpp/vccore.cpp", "-o"]).arg(&obj));
    run(Command::new(ar).arg("crs").arg(out.join("libvccore.a")).arg(&obj));
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=vccore");
    println!("cargo:rerun-if-changed=cpp/vccore.cpp");
}
