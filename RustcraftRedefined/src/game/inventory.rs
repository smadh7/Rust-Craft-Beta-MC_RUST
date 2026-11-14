//! Player inventory and the slot-click rules shared by every container screen.

use crate::item::{ItemId, ItemStack};

pub const HOTBAR: usize = 9;
pub const SLOTS: usize = 36;

#[derive(Clone)]
pub struct Inventory {
    /// Slots 0-8 are the hotbar, 9-35 the main inventory.
    pub slots: [ItemStack; SLOTS],
    pub selected: usize,
}

impl Default for Inventory {
    fn default() -> Self {
        Inventory { slots: [ItemStack::EMPTY; SLOTS], selected: 0 }
    }
}

impl Inventory {
    pub fn held(&self) -> ItemStack {
        self.slots[self.selected]
    }
    pub fn held_mut(&mut self) -> &mut ItemStack {
        &mut self.slots[self.selected]
    }
    pub fn count(&self, id: ItemId) -> u32 {
        self.slots.iter().filter(|s| s.id == id).map(|s| s.count as u32).sum()
    }
    pub fn clear(&mut self) {
        self.slots = [ItemStack::EMPTY; SLOTS];
    }
    /// Adds a stack, filling matching stacks first; returns what did not fit.
    pub fn add(&mut self, stack: ItemStack) -> ItemStack {
        add_to(&mut self.slots, stack)
    }
    /// Adds into a sub-range of slots only (used by shift-click).
    pub fn add_range(&mut self, stack: ItemStack, from: usize, to: usize) -> ItemStack {
        add_to(&mut self.slots[from..to], stack)
    }
    /// Removes up to `n` items of a kind; returns how many were removed.
    pub fn remove(&mut self, id: ItemId, mut n: u32) -> u32 {
        let want = n;
        for s in self.slots.iter_mut() {
            if s.id == id && n > 0 {
                let take = (s.count as u32).min(n);
                s.take(take as u8);
                n -= take;
            }
        }
        want - n
    }
    /// Removes one item from the selected slot (placing a block, eating).
    pub fn consume_held(&mut self) {
        self.slots[self.selected].take(1);
    }
}

/// Merges `stack` into `slots`: existing stacks first, then empty slots.  Returns the remainder.
pub fn add_to(slots: &mut [ItemStack], mut stack: ItemStack) -> ItemStack {
    if stack.is_empty() {
        return ItemStack::EMPTY;
    }
    let max = stack.max();
    for s in slots.iter_mut() {
        if !s.is_empty() && s.stacks_with(&stack) && s.count < max {
            let n = (max - s.count).min(stack.count);
            s.count += n;
            stack.count -= n;
            if stack.count == 0 {
                return ItemStack::EMPTY;
            }
        }
    }
    for s in slots.iter_mut() {
        if s.is_empty() {
            let n = max.min(stack.count);
            *s = ItemStack { id: stack.id, count: n, damage: stack.damage };
            stack.count -= n;
            if stack.count == 0 {
                return ItemStack::EMPTY;
            }
        }
    }
    stack
}

/// Left click on an ordinary slot with `cursor` being the stack carried by the mouse.
pub fn click_left(slot: &mut ItemStack, cursor: &mut ItemStack) {
    if cursor.is_empty() {
        *cursor = std::mem::take(slot);
    } else if slot.is_empty() {
        *slot = std::mem::take(cursor);
    } else if slot.stacks_with(cursor) {
        let n = (slot.max() - slot.count).min(cursor.count);
        slot.count += n;
        cursor.take(n);
    } else {
        std::mem::swap(slot, cursor);
    }
}

/// Right click: pick up half a stack, or put down a single item.
pub fn click_right(slot: &mut ItemStack, cursor: &mut ItemStack) {
    if cursor.is_empty() {
        let half = slot.count.div_ceil(2);
        *cursor = slot.take(half);
    } else if slot.is_empty() {
        *slot = cursor.take(1);
    } else if slot.stacks_with(cursor) {
        if slot.count < slot.max() {
            slot.count += 1;
            cursor.take(1);
        }
    } else {
        std::mem::swap(slot, cursor);
    }
}

/// Taking from an output slot (crafting result, furnace output): only whole results, never placing.
/// Returns true when the result was taken.
pub fn take_output(output: &mut ItemStack, cursor: &mut ItemStack) -> bool {
    if output.is_empty() {
        return false;
    }
    if cursor.is_empty() {
        *cursor = std::mem::take(output);
        true
    } else if cursor.stacks_with(output) && cursor.count as u32 + output.count as u32 <= cursor.max() as u32 {
        cursor.count += output.count;
        *output = ItemStack::EMPTY;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(name: &str, n: u8) -> ItemStack {
        ItemStack::named(name, n)
    }

    #[test]
    fn adding_merges_then_fills() {
        let mut inv = Inventory::default();
        assert!(inv.add(s("dirt", 40)).is_empty());
        assert!(inv.add(s("dirt", 40)).is_empty());
        assert_eq!(inv.slots[0], s("dirt", 64));
        assert_eq!(inv.slots[1], s("dirt", 16));
        assert_eq!(inv.count(s("dirt", 1).id), 80);
        // Tools do not stack.
        inv.add(s("iron_pickaxe", 1));
        inv.add(s("iron_pickaxe", 1));
        assert_eq!(inv.slots[2].count, 1);
        assert_eq!(inv.slots[3].count, 1);
        assert_eq!(inv.remove(s("dirt", 1).id, 70), 70);
        assert_eq!(inv.count(s("dirt", 1).id), 10);
        // A full inventory returns the remainder.
        let mut full = Inventory::default();
        for slot in full.slots.iter_mut() {
            *slot = s("stone", 64);
        }
        assert_eq!(full.add(s("dirt", 5)), s("dirt", 5));
        assert_eq!(full.add(s("stone", 5)), s("stone", 5));
    }

    #[test]
    fn click_rules() {
        let (mut slot, mut cursor) = (s("stone", 10), ItemStack::EMPTY);
        click_right(&mut slot, &mut cursor);
        assert_eq!((slot, cursor), (s("stone", 5), s("stone", 5)));
        click_left(&mut slot, &mut cursor);
        assert_eq!((slot, cursor), (s("stone", 10), ItemStack::EMPTY));
        let (mut slot, mut cursor) = (s("stone", 60), s("stone", 10));
        click_left(&mut slot, &mut cursor);
        assert_eq!((slot, cursor), (s("stone", 64), s("stone", 6)));
        let (mut slot, mut cursor) = (s("stone", 3), s("dirt", 2));
        click_left(&mut slot, &mut cursor);
        assert_eq!((slot, cursor), (s("dirt", 2), s("stone", 3)));
        let (mut slot, mut cursor) = (ItemStack::EMPTY, s("dirt", 2));
        click_right(&mut slot, &mut cursor);
        assert_eq!((slot, cursor), (s("dirt", 1), s("dirt", 1)));
        // Output slots only give.
        let (mut out, mut cursor) = (s("stick", 4), s("stick", 62));
        assert!(!take_output(&mut out, &mut cursor));
        cursor.count = 60;
        assert!(take_output(&mut out, &mut cursor));
        assert_eq!(cursor.count, 64);
        assert!(out.is_empty());
    }
}
