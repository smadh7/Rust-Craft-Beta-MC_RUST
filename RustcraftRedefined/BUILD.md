# Build and tests

The standalone prototype can be built offline with an already installed Zig toolchain containing its Windows GNU cross compiler and libraries. Run `./build-windows.sh` from this folder. It produces `RustcraftRedefined.exe` in the parent folder. Inspect the executable imports before distributing it; the current Zig build imports Universal C Runtime DLLs, which do not meet the original restricted import requirement. The toolchain itself is not included in this source folder.

For a native C++ smoke test and benchmark, run:

    g++ -O3 -ffast-math -fno-exceptions -fno-rtti -DNDEBUG -std=c++17 cpp/vccore.cpp tests/core_test.cpp -o core-test
    ./core-test

The separate, unused Rust foundation can be tested with an already installed Rust and C++ toolchain using `cargo test --offline` and `cargo run --offline --release --bin bench`. No third-party Rust crates are required. The Rust binary is a placeholder and is not the C++ client.

For the independent camera, raycast and placement tests:

    g++ -O3 -std=c++17 tests/interaction_test.cpp -o interaction-test
    ./interaction-test

These native tests do not replace Windows, graphics-driver, input or gameplay testing.
