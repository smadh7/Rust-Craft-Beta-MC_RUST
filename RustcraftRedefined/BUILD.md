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

For name, seed and world-header validation tests:

    g++ -O2 -std=c++17 tests/world_menu_test.cpp -o world-menu-test
    ./world-menu-test

The Win32 startup menu, save migration, actual graphics and user input still require testing on Windows.

For gameplay and save-format tests (assertions must remain enabled):

    g++ -O3 -ffast-math -std=c++17 tests/gameplay_test.cpp -o gameplay-test
    ./gameplay-test

The game modes, damage, persistence, menu, and migration need end-to-end Windows testing; native unit tests do not establish parity or gameplay correctness.

For visual-effect math and water meshing tests (keep assertions enabled):

    g++ -O3 -ffast-math -std=c++17 tests/visual_effects_test.cpp -o visual-effects-test
    ./visual-effects-test
    g++ -O3 -ffast-math -std=c++17 cpp/vccore.cpp tests/water_mesh_test.cpp -o water-mesh-test
    ./water-mesh-test

The animated surface, particles, profile transitions and mining overlay still need visual testing on Windows and representative GPUs.

For indexed edit-store, save-record reconstruction, frustum safety and synthetic lookup benchmarks (assertions enabled):

    g++ -O3 -ffast-math -std=c++17 tests/performance_test.cpp -o performance-test
    ./performance-test

The lookup timings are a synthetic CPU microbenchmark, not a measurement of frame rate or a comparison with another game. The inventory picker, save migration and culling must still be tested in the Windows client.
