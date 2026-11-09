#!/bin/sh
set -eu
cd "$(dirname "$0")"
zig c++ -target x86_64-windows-gnu -O3 -ffast-math -fno-exceptions -fno-rtti -DNDEBUG -std=c++17 cpp/client.cpp cpp/menu.cpp cpp/vccore.cpp -lopengl32 -lgdi32 -luser32 -Wl,--subsystem,windows -s -o ../RustcraftRedefined.exe
printf '%s\n' 'Built an experimental, untested prototype. Check DLL imports before distribution.'
