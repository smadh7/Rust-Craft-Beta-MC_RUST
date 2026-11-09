#ifndef RUSTCRAFT_INVENTORY_H
#define RUSTCRAFT_INVENTORY_H
#include <windows.h>
#include "gameplay.h"
// Returns a selected block ID, or zero when cancelled. No inventory state is
// changed here; the caller assigns the result to the active hotbar slot.
uint8_t open_block_inventory(HWND parent,HINSTANCE instance,const PlayerState& player,int slot,uint8_t current);
#endif
