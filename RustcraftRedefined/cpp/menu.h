#ifndef RUSTCRAFT_MENU_H
#define RUSTCRAFT_MENU_H
#include <windows.h>
#include <stdint.h>
#include <string>
#include "gameplay.h"
struct MenuResult {
    bool start=false;
    uint64_t seed=12345;
    int graphics=1;
    int skin=0;
    GameMode mode=CREATIVE;
    std::wstring save_path;
    std::wstring data_dir;
    std::wstring world_name;
};
MenuResult open_start_menu(HINSTANCE instance);
void save_preferences(const MenuResult& state);
#endif
