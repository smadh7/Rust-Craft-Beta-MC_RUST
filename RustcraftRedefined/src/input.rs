//! Platform-independent input events and per-frame input state.

/// Key codes (printable keys use their upper-case ASCII value).
pub mod key {
    pub const SPACE: u32 = 32;
    pub const SLASH: u32 = 47;
    pub const N0: u32 = 48;
    pub const N1: u32 = 49;
    pub const N9: u32 = 57;
    pub const B: u32 = 66;
    pub const A: u32 = 65;
    pub const C: u32 = 67;
    pub const D: u32 = 68;
    pub const E: u32 = 69;
    pub const F: u32 = 70;
    pub const G: u32 = 71;
    pub const O: u32 = 79;
    pub const Q: u32 = 81;
    pub const R: u32 = 82;
    pub const S: u32 = 83;
    pub const T: u32 = 84;
    pub const V: u32 = 86;
    pub const W: u32 = 87;
    pub const ESCAPE: u32 = 256;
    pub const ENTER: u32 = 257;
    pub const TAB: u32 = 258;
    pub const BACKSPACE: u32 = 259;
    pub const DELETE: u32 = 261;
    pub const RIGHT: u32 = 262;
    pub const LEFT: u32 = 263;
    pub const DOWN: u32 = 264;
    pub const UP: u32 = 265;
    pub const F1: u32 = 290;
    pub const F2: u32 = 291;
    pub const F3: u32 = 292;
    pub const F4: u32 = 293;
    pub const F5: u32 = 294;
    pub const F11: u32 = 300;
    pub const SHIFT: u32 = 340;
    pub const CONTROL: u32 = 341;
    pub const ALT: u32 = 342;
    pub const COUNT: usize = 352;
}

pub const MOUSE_LEFT: usize = 0;
pub const MOUSE_RIGHT: usize = 1;
pub const MOUSE_MIDDLE: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Key { code: u32, down: bool },
    Char(char),
    /// Relative motion while the cursor is captured.
    MouseMove { dx: f32, dy: f32 },
    /// Absolute cursor position in window pixels.
    MousePos { x: f32, y: f32 },
    MouseButton { button: usize, down: bool },
    Wheel(f32),
    Resize(i32, i32),
    Focus(bool),
    /// The platform captured or released the cursor (pointer lock).
    Grab(bool),
}

pub struct Input {
    down: [bool; key::COUNT],
    pressed: [bool; key::COUNT],
    pub mouse_down: [bool; 3],
    pub mouse_pressed: [bool; 3],
    pub mouse_released: [bool; 3],
    pub look: (f32, f32),
    pub cursor: (f32, f32),
    pub wheel: f32,
    pub typed: Vec<char>,
    /// Every key-down event of the frame including auto-repeats (for text editing).
    pub key_events: Vec<u32>,
}

impl Default for Input {
    fn default() -> Self {
        Input { down: [false; key::COUNT], pressed: [false; key::COUNT], mouse_down: [false; 3], mouse_pressed: [false; 3], mouse_released: [false; 3], look: (0.0, 0.0), cursor: (0.0, 0.0), wheel: 0.0, typed: Vec::new(), key_events: Vec::new() }
    }
}

impl Input {
    pub fn handle(&mut self, ev: &Event) {
        match *ev {
            Event::Key { code, down } => {
                if down {
                    self.key_events.push(code);
                }
                if let Some(slot) = self.down.get_mut(code as usize) {
                    if down && !*slot {
                        self.pressed[code as usize] = true;
                    }
                    *slot = down;
                }
            }
            Event::Char(c) => self.typed.push(c),
            Event::MouseMove { dx, dy } => {
                self.look.0 += dx;
                self.look.1 += dy;
            }
            Event::MousePos { x, y } => self.cursor = (x, y),
            Event::MouseButton { button, down } => {
                if button < 3 {
                    if down && !self.mouse_down[button] {
                        self.mouse_pressed[button] = true;
                    }
                    if !down && self.mouse_down[button] {
                        self.mouse_released[button] = true;
                    }
                    self.mouse_down[button] = down;
                }
            }
            Event::Wheel(d) => self.wheel += d,
            Event::Focus(false) | Event::Grab(false) => self.release_all(),
            _ => {}
        }
    }
    #[inline]
    pub fn down(&self, code: u32) -> bool {
        self.down.get(code as usize).copied().unwrap_or(false)
    }
    /// True once for the frame in which the key went down.
    #[inline]
    pub fn pressed(&self, code: u32) -> bool {
        self.pressed.get(code as usize).copied().unwrap_or(false)
    }
    pub fn release_all(&mut self) {
        self.down = [false; key::COUNT];
        self.mouse_down = [false; 3];
    }
    /// Clears the per-frame edges; call at the end of every frame.
    pub fn end_frame(&mut self) {
        self.pressed = [false; key::COUNT];
        self.mouse_pressed = [false; 3];
        self.mouse_released = [false; 3];
        self.look = (0.0, 0.0);
        self.wheel = 0.0;
        self.typed.clear();
        self.key_events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_last_one_frame() {
        let mut i = Input::default();
        i.handle(&Event::Key { code: key::W, down: true });
        assert!(i.down(key::W) && i.pressed(key::W));
        i.handle(&Event::Key { code: key::W, down: true }); // key repeat
        i.end_frame();
        assert!(i.down(key::W) && !i.pressed(key::W));
        i.handle(&Event::Focus(false));
        assert!(!i.down(key::W));
        i.handle(&Event::Key { code: 9999, down: true });
        assert!(!i.down(9999));
    }
}
