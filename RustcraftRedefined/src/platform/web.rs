//! Browser back end: JavaScript imports for services and the exported entry points the page calls.

use crate::game::Game;
use crate::input::Event;
use std::cell::RefCell;

#[link(wasm_import_module = "env")]
extern "C" {
    fn js_log(ptr: *const u8, len: usize);
    fn js_now() -> f64;
    fn js_grab(on: u32);
    fn js_storage_get(key: *const u8, key_len: usize) -> i32;
    fn js_storage_take(dst: *mut u8);
    fn js_storage_set(key: *const u8, key_len: usize, data: *const u8, len: usize);
    fn js_storage_remove(key: *const u8, key_len: usize);
    fn js_storage_keys(prefix: *const u8, prefix_len: usize) -> i32;
    fn js_download(name: *const u8, name_len: usize, data: *const u8, len: usize);
    fn js_sound_load(id: u32, samples: *const i16, len: usize, rate: u32);
    fn js_sound_play(id: u32, volume: f32, pitch: f32);
}

// SAFETY (all wrappers): pointer/length pairs stay valid for the synchronous call.
pub fn log(msg: &str) {
    unsafe { js_log(msg.as_ptr(), msg.len()) }
}
pub fn now_ms() -> f64 {
    unsafe { js_now() }
}
pub fn set_cursor_grab(on: bool) {
    unsafe { js_grab(on as u32) }
}
pub fn request_quit() {}
pub fn storage_read(key: &str) -> Option<Vec<u8>> {
    unsafe {
        let n = js_storage_get(key.as_ptr(), key.len());
        if n < 0 {
            return None;
        }
        let mut buf = vec![0u8; n as usize];
        js_storage_take(buf.as_mut_ptr());
        Some(buf)
    }
}
pub fn storage_write(key: &str, data: &[u8]) {
    unsafe { js_storage_set(key.as_ptr(), key.len(), data.as_ptr(), data.len()) }
}
pub fn storage_remove(key: &str) {
    unsafe { js_storage_remove(key.as_ptr(), key.len()) }
}
pub fn storage_list(prefix: &str) -> Vec<String> {
    unsafe {
        let n = js_storage_keys(prefix.as_ptr(), prefix.len());
        if n <= 0 {
            return Vec::new();
        }
        let mut buf = vec![0u8; n as usize];
        js_storage_take(buf.as_mut_ptr());
        String::from_utf8_lossy(&buf).split('\n').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()
    }
}
/// Registers a sound effect (16-bit mono samples) under an id.
pub fn sound_load(id: u32, samples: &[i16], rate: u32) {
    unsafe { js_sound_load(id, samples.as_ptr(), samples.len(), rate) }
}
pub fn sound_play(id: u32, volume: f32, pitch: f32) {
    unsafe { js_sound_play(id, volume, pitch) }
}
pub fn export_file(name: &str, data: &[u8]) {
    unsafe { js_download(name.as_ptr(), name.len(), data.as_ptr(), data.len()) }
}

thread_local! {
    static GAME: RefCell<Option<Game>> = const { RefCell::new(None) };
}

fn with_game(f: impl FnOnce(&mut Game)) {
    GAME.with(|g| {
        if let Ok(mut slot) = g.try_borrow_mut() {
            if let Some(game) = slot.as_mut() {
                f(game);
            }
        }
    });
}

#[no_mangle]
pub extern "C" fn rc_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `ptr` must come from `rc_alloc` with the same `len`.
#[no_mangle]
pub unsafe extern "C" fn rc_free(ptr: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(ptr, 0, len.max(1)));
}

#[no_mangle]
pub extern "C" fn rc_init(width: i32, height: i32) {
    std::panic::set_hook(Box::new(|info| log(&format!("PANIC: {info}"))));
    let game = Game::new(width, height);
    GAME.with(|g| *g.borrow_mut() = Some(game));
}

#[no_mangle]
pub extern "C" fn rc_frame(now_ms: f64) {
    with_game(|g| g.frame(now_ms));
}

/// Runs a chat command (without the leading slash).
/// # Safety
/// `ptr`/`len` must describe valid UTF-8 bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_command(ptr: *const u8, len: usize) {
    let text = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    with_game(|g| g.run_command(&text));
}

/// Test and tooling hook: 0 loading flag, 1 fps, 2..4 player position, 5 meshed chunks, 6 loaded chunks.
#[no_mangle]
pub extern "C" fn rc_query(what: u32) -> f64 {
    let mut out = -1.0;
    with_game(|g| out = g.query(what));
    out
}

/// Writes the open world and the options to storage (the page calls this when it is hidden or closed).
#[no_mangle]
pub extern "C" fn rc_save() {
    with_game(|g| g.persist());
}

#[no_mangle]
pub extern "C" fn rc_resize(w: i32, h: i32) {
    with_game(|g| g.handle(Event::Resize(w, h)));
}
#[no_mangle]
pub extern "C" fn rc_key(code: u32, down: u32) {
    with_game(|g| g.handle(Event::Key { code, down: down != 0 }));
}
#[no_mangle]
pub extern "C" fn rc_char(codepoint: u32) {
    if let Some(c) = char::from_u32(codepoint) {
        with_game(|g| g.handle(Event::Char(c)));
    }
}
#[no_mangle]
pub extern "C" fn rc_mouse_move(dx: f32, dy: f32) {
    with_game(|g| g.handle(Event::MouseMove { dx, dy }));
}
#[no_mangle]
pub extern "C" fn rc_mouse_pos(x: f32, y: f32) {
    with_game(|g| g.handle(Event::MousePos { x, y }));
}
#[no_mangle]
pub extern "C" fn rc_mouse_button(button: u32, down: u32) {
    with_game(|g| g.handle(Event::MouseButton { button: button as usize, down: down != 0 }));
}
#[no_mangle]
pub extern "C" fn rc_wheel(delta: f32) {
    with_game(|g| g.handle(Event::Wheel(delta)));
}
#[no_mangle]
pub extern "C" fn rc_focus(on: u32) {
    with_game(|g| g.handle(Event::Focus(on != 0)));
}
#[no_mangle]
pub extern "C" fn rc_grab(on: u32) {
    with_game(|g| g.handle(Event::Grab(on != 0)));
}
