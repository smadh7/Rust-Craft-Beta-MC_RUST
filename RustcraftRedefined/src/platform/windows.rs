//! Windows desktop back end: a Win32 window with an OpenGL 3.3 core context created through WGL.
//! Everything is declared by hand; no bindings crate is used.
#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

use super::native::{cursor_grab_requested, log, now_ms, quit_requested, request_quit};
use crate::game::Game;
use crate::gfx::gl;
use crate::input::{key, Event};
use std::cell::RefCell;
use std::ffi::{c_char, c_void, CString};
use std::ptr::{null, null_mut};

type HANDLE = *mut c_void;
type HWND = HANDLE;
type HDC = HANDLE;
type HGLRC = HANDLE;
type WPARAM = usize;
type LPARAM = isize;
type LRESULT = isize;
type BOOL = i32;
type WNDPROC = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

#[repr(C)]
struct WNDCLASSEXW {
    cbSize: u32,
    style: u32,
    lpfnWndProc: Option<WNDPROC>,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: HANDLE,
    hIcon: HANDLE,
    hCursor: HANDLE,
    hbrBackground: HANDLE,
    lpszMenuName: *const u16,
    lpszClassName: *const u16,
    hIconSm: HANDLE,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct POINT {
    x: i32,
    y: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct MSG {
    hwnd: HWND,
    message: u32,
    wParam: WPARAM,
    lParam: LPARAM,
    time: u32,
    pt: POINT,
}
#[repr(C)]
#[derive(Default)]
struct PIXELFORMATDESCRIPTOR {
    nSize: u16,
    nVersion: u16,
    dwFlags: u32,
    iPixelType: u8,
    cColorBits: u8,
    cRedBits: u8,
    cRedShift: u8,
    cGreenBits: u8,
    cGreenShift: u8,
    cBlueBits: u8,
    cBlueShift: u8,
    cAlphaBits: u8,
    cAlphaShift: u8,
    cAccumBits: u8,
    cAccumRedBits: u8,
    cAccumGreenBits: u8,
    cAccumBlueBits: u8,
    cAccumAlphaBits: u8,
    cDepthBits: u8,
    cStencilBits: u8,
    cAuxBuffers: u8,
    iLayerType: u8,
    bReserved: u8,
    dwLayerMask: u32,
    dwVisibleMask: u32,
    dwDamageMask: u32,
}
#[repr(C)]
#[derive(Default)]
struct MONITORINFO {
    cbSize: u32,
    rcMonitor: RECT,
    rcWork: RECT,
    dwFlags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct WINDOWPLACEMENT {
    length: u32,
    flags: u32,
    showCmd: u32,
    ptMinPosition: POINT,
    ptMaxPosition: POINT,
    rcNormalPosition: RECT,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassExW(class: *const WNDCLASSEXW) -> u16;
    fn CreateWindowExW(ex_style: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: HWND, menu: HANDLE, instance: HANDLE, param: *mut c_void) -> HWND;
    fn DefWindowProcW(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT;
    fn DestroyWindow(hwnd: HWND) -> BOOL;
    fn ShowWindow(hwnd: HWND, cmd: i32) -> BOOL;
    fn PeekMessageW(msg: *mut MSG, hwnd: HWND, min: u32, max: u32, remove: u32) -> BOOL;
    fn TranslateMessage(msg: *const MSG) -> BOOL;
    fn DispatchMessageW(msg: *const MSG) -> LRESULT;
    fn PostQuitMessage(code: i32);
    fn GetDC(hwnd: HWND) -> HDC;
    fn LoadCursorW(instance: HANDLE, name: *const u16) -> HANDLE;
    fn SetCursor(cursor: HANDLE) -> HANDLE;
    fn ShowCursor(show: BOOL) -> i32;
    fn GetClientRect(hwnd: HWND, rect: *mut RECT) -> BOOL;
    fn ClientToScreen(hwnd: HWND, point: *mut POINT) -> BOOL;
    fn SetCursorPos(x: i32, y: i32) -> BOOL;
    fn GetCursorPos(point: *mut POINT) -> BOOL;
    fn ClipCursor(rect: *const RECT) -> BOOL;
    fn GetForegroundWindow() -> HWND;
    fn SetCapture(hwnd: HWND) -> HWND;
    fn ReleaseCapture() -> BOOL;
    fn GetWindowLongPtrW(hwnd: HWND, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: HWND, index: i32, value: isize) -> isize;
    fn SetWindowPos(hwnd: HWND, after: HWND, x: i32, y: i32, w: i32, h: i32, flags: u32) -> BOOL;
    fn GetWindowPlacement(hwnd: HWND, placement: *mut WINDOWPLACEMENT) -> BOOL;
    fn SetWindowPlacement(hwnd: HWND, placement: *const WINDOWPLACEMENT) -> BOOL;
    fn MonitorFromWindow(hwnd: HWND, flags: u32) -> HANDLE;
    fn GetMonitorInfoW(monitor: HANDLE, info: *mut MONITORINFO) -> BOOL;
    fn MessageBoxW(hwnd: HWND, text: *const u16, caption: *const u16, kind: u32) -> i32;
    fn AdjustWindowRect(rect: *mut RECT, style: u32, menu: BOOL) -> BOOL;
    fn SetProcessDPIAware() -> BOOL;
}
#[link(name = "winmm")]
extern "system" {
    fn PlaySoundW(sound: *const u16, module: HANDLE, flags: u32) -> BOOL;
}

/// WAV images of the sound effects.  They are leaked on purpose: Windows reads them while a
/// sound plays, so they must never move or be freed.
static SOUNDS: std::sync::Mutex<Vec<Option<(&'static [u8], f64)>>> = std::sync::Mutex::new(Vec::new());
/// When the sound now playing ends, and how loud it was asked to be.
static PLAYING: std::sync::Mutex<(f64, f32)> = std::sync::Mutex::new((0.0, 0.0));

pub fn sound_load(id: u32, samples: &[i16], rate: u32) {
    let data_len = samples.len() as u32 * 2;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        // Quieter than full scale: the simple player has no volume control.
        wav.extend_from_slice(&(s / 2).to_le_bytes());
    }
    let image: &'static [u8] = Box::leak(wav.into_boxed_slice());
    let millis = samples.len() as f64 * 1000.0 / rate as f64;
    if let Ok(mut table) = SOUNDS.lock() {
        let i = id as usize;
        if table.len() <= i {
            table.resize(i + 1, None);
        }
        table[i] = Some((image, millis));
    }
}

/// Plays a sound.  Only one can sound at a time here, so a quieter request does not cut
/// off a louder sound that is still playing.
pub fn sound_play(id: u32, volume: f32) {
    let Some((image, millis)) = SOUNDS.lock().ok().and_then(|t| t.get(id as usize).copied().flatten()) else { return };
    let now = now_ms();
    let Ok(mut playing) = PLAYING.lock() else { return };
    if now < playing.0 && volume < playing.1 {
        return;
    }
    *playing = (now + millis, volume);
    // SAFETY: the image is a complete WAV file that lives for the rest of the program.
    // Flags: SND_ASYNC | SND_NODEFAULT | SND_MEMORY.
    unsafe {
        PlaySoundW(image.as_ptr() as *const u16, null_mut(), 0x0001 | 0x0002 | 0x0004);
    }
}

#[link(name = "gdi32")]
extern "system" {
    fn ChoosePixelFormat(dc: HDC, pfd: *const PIXELFORMATDESCRIPTOR) -> i32;
    fn SetPixelFormat(dc: HDC, format: i32, pfd: *const PIXELFORMATDESCRIPTOR) -> BOOL;
    fn SwapBuffers(dc: HDC) -> BOOL;
}
#[link(name = "opengl32")]
extern "system" {
    fn wglCreateContext(dc: HDC) -> HGLRC;
    fn wglMakeCurrent(dc: HDC, rc: HGLRC) -> BOOL;
    fn wglDeleteContext(rc: HGLRC) -> BOOL;
    fn wglGetProcAddress(name: *const c_char) -> *const c_void;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> HANDLE;
    fn LoadLibraryA(name: *const c_char) -> HANDLE;
    fn GetProcAddress(module: HANDLE, name: *const c_char) -> *const c_void;
}

const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_POPUP: u32 = 0x8000_0000;
const CS_OWNDC: u32 = 0x0020;
const CW_USEDEFAULT: i32 = i32::MIN;
const GWL_STYLE: i32 = -16;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOOWNERZORDER: u32 = 0x0200;
const SWP_FRAMECHANGED: u32 = 0x0020;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOSIZE: u32 = 0x0001;
const PM_REMOVE: u32 = 1;
const WM_QUIT: u32 = 0x0012;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

thread_local! {
    static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    static GRABBED: RefCell<bool> = const { RefCell::new(false) };
}

fn push(ev: Event) {
    EVENTS.with(|q| {
        if let Ok(mut q) = q.try_borrow_mut() {
            q.push(ev);
        }
    });
}

fn map_key(vk: usize) -> u32 {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => vk as u32,
        0x20 => key::SPACE,
        0x1B => key::ESCAPE,
        0x0D => key::ENTER,
        0x09 => key::TAB,
        0x08 => key::BACKSPACE,
        0x2E => key::DELETE,
        0x27 => key::RIGHT,
        0x25 => key::LEFT,
        0x28 => key::DOWN,
        0x26 => key::UP,
        0x70..=0x7B => key::F1 + (vk as u32 - 0x70),
        0x10 | 0xA0 | 0xA1 => key::SHIFT,
        0x11 | 0xA2 | 0xA3 => key::CONTROL,
        0x12 | 0xA4 | 0xA5 => key::ALT,
        0xBF => key::SLASH,
        0xBD => 45,
        0xBB => 61,
        0xBC => 44,
        0xBE => 46,
        _ => 0,
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let lo = (l & 0xffff) as i16 as i32;
    let hi = ((l >> 16) & 0xffff) as i16 as i32;
    match msg {
        0x0010 => {
            // WM_CLOSE
            request_quit();
            return 0;
        }
        0x0002 => {
            // WM_DESTROY
            PostQuitMessage(0);
            return 0;
        }
        0x0005 => {
            // WM_SIZE
            let (cw, ch) = ((l & 0xffff) as i32, ((l >> 16) & 0xffff) as i32);
            if cw > 0 && ch > 0 {
                push(Event::Resize(cw, ch));
            }
            return 0;
        }
        0x0007 => push(Event::Focus(true)),
        0x0008 => push(Event::Focus(false)),
        0x0014 => return 1, // WM_ERASEBKGND: the game repaints everything
        0x0020 => {
            // WM_SETCURSOR: hide the arrow while the cursor is captured
            if (l & 0xffff) == 1 && GRABBED.with(|g| *g.borrow()) {
                SetCursor(null_mut());
                return 1;
            }
        }
        0x0100 | 0x0104 => {
            let code = map_key(w);
            if code != 0 {
                push(Event::Key { code, down: true });
            }
            // Keep Alt+F4 working; swallow other system keys so Alt/F10 do not open the window menu.
            if msg == 0x0104 && w != 0x73 {
                return 0;
            }
        }
        0x0101 | 0x0105 => {
            let code = map_key(w);
            if code != 0 {
                push(Event::Key { code, down: false });
            }
            if msg == 0x0105 {
                return 0;
            }
        }
        0x0102 => {
            // WM_CHAR
            if let Some(c) = char::from_u32(w as u32) {
                if !c.is_control() {
                    push(Event::Char(c));
                }
            }
            return 0;
        }
        0x0200 => push(Event::MousePos { x: lo as f32, y: hi as f32 }),
        0x0201 | 0x0204 | 0x0207 => {
            SetCapture(hwnd);
            push(Event::MouseButton { button: match msg { 0x0201 => 0, 0x0204 => 1, _ => 2 }, down: true });
            return 0;
        }
        0x0202 | 0x0205 | 0x0208 => {
            ReleaseCapture();
            push(Event::MouseButton { button: match msg { 0x0202 => 0, 0x0205 => 1, _ => 2 }, down: false });
            return 0;
        }
        0x020A => {
            // WM_MOUSEWHEEL: positive delta scrolls away from the user (previous slot).
            let delta = ((w >> 16) & 0xffff) as i16 as f32 / 120.0;
            push(Event::Wheel(-delta));
            return 0;
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, w, l)
}

/// Shows an error dialog and writes the message to the log.
pub fn fatal(message: &str) {
    log(&format!("FATAL: {message}"));
    // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive the call.
    unsafe {
        MessageBoxW(null_mut(), wide(message).as_ptr(), wide("Rustcraft Redefined").as_ptr(), 0x10);
    }
}

struct Window {
    hwnd: HWND,
    dc: HDC,
    fullscreen: bool,
    saved: WINDOWPLACEMENT,
    grabbed: bool,
}

impl Window {
    fn client_size(&self) -> (i32, i32) {
        let mut r = RECT::default();
        // SAFETY: valid window handle and out pointer.
        unsafe { GetClientRect(self.hwnd, &mut r) };
        ((r.right - r.left).max(1), (r.bottom - r.top).max(1))
    }

    fn centre_on_screen(&self) -> POINT {
        let (w, h) = self.client_size();
        let mut p = POINT { x: w / 2, y: h / 2 };
        // SAFETY: valid window handle and in/out pointer.
        unsafe { ClientToScreen(self.hwnd, &mut p) };
        p
    }

    fn set_grab(&mut self, on: bool) {
        if on == self.grabbed {
            return;
        }
        self.grabbed = on;
        GRABBED.with(|g| *g.borrow_mut() = on);
        // SAFETY: plain Win32 calls with valid arguments.
        unsafe {
            if on {
                let (w, h) = self.client_size();
                let mut tl = POINT { x: 0, y: 0 };
                ClientToScreen(self.hwnd, &mut tl);
                let clip = RECT { left: tl.x, top: tl.y, right: tl.x + w, bottom: tl.y + h };
                ClipCursor(&clip);
                ShowCursor(0);
                let c = self.centre_on_screen();
                SetCursorPos(c.x, c.y);
            } else {
                ClipCursor(null());
                ShowCursor(1);
            }
        }
        push(Event::Grab(on));
    }

    /// Relative mouse motion while captured: measure the offset from the centre, then re-centre.
    fn poll_mouse(&self) {
        if !self.grabbed {
            return;
        }
        let c = self.centre_on_screen();
        let mut p = POINT::default();
        // SAFETY: valid out pointer.
        unsafe {
            if GetCursorPos(&mut p) != 0 && (p.x != c.x || p.y != c.y) {
                push(Event::MouseMove { dx: (p.x - c.x) as f32, dy: (p.y - c.y) as f32 });
                SetCursorPos(c.x, c.y);
            }
        }
    }

    fn toggle_fullscreen(&mut self) {
        let was_grabbed = self.grabbed;
        self.set_grab(false);
        // SAFETY: documented borderless-fullscreen sequence using this window's own handle.
        unsafe {
            let style = GetWindowLongPtrW(self.hwnd, GWL_STYLE) as u32;
            if !self.fullscreen {
                let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
                self.saved.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
                if GetWindowPlacement(self.hwnd, &mut self.saved) != 0 && GetMonitorInfoW(MonitorFromWindow(self.hwnd, 1), &mut mi) != 0 {
                    SetWindowLongPtrW(self.hwnd, GWL_STYLE, ((style & !WS_OVERLAPPEDWINDOW) | WS_POPUP) as isize);
                    let m = mi.rcMonitor;
                    SetWindowPos(self.hwnd, null_mut(), m.left, m.top, m.right - m.left, m.bottom - m.top, SWP_NOOWNERZORDER | SWP_FRAMECHANGED);
                    self.fullscreen = true;
                }
            } else {
                SetWindowLongPtrW(self.hwnd, GWL_STYLE, ((style & !WS_POPUP) | WS_OVERLAPPEDWINDOW) as isize);
                SetWindowPlacement(self.hwnd, &self.saved);
                SetWindowPos(self.hwnd, null_mut(), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED);
                self.fullscreen = false;
            }
        }
        self.set_grab(was_grabbed);
    }
}

/// Creates the window and an OpenGL 3.3 core context, then loads the GL entry points.
unsafe fn create_window() -> Result<Window, String> {
    SetProcessDPIAware();
    let instance = GetModuleHandleW(null());
    let class_name = wide("RustcraftRedefinedWindow");
    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_OWNDC,
        lpfnWndProc: Some(wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: null_mut(),
        hCursor: LoadCursorW(null_mut(), 32512 as *const u16),
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: null_mut(),
    };
    if RegisterClassExW(&class) == 0 {
        return Err("Could not register the window class.".into());
    }
    let style = WS_OVERLAPPEDWINDOW;
    let mut rect = RECT { left: 0, top: 0, right: 1280, bottom: 720 };
    AdjustWindowRect(&mut rect, style, 0);
    let hwnd = CreateWindowExW(0, class_name.as_ptr(), wide("Rustcraft Redefined").as_ptr(), style, CW_USEDEFAULT, CW_USEDEFAULT, rect.right - rect.left, rect.bottom - rect.top, null_mut(), null_mut(), instance, null_mut());
    if hwnd.is_null() {
        return Err("Could not create the game window.".into());
    }
    let dc = GetDC(hwnd);
    let pfd = PIXELFORMATDESCRIPTOR {
        nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
        nVersion: 1,
        dwFlags: 0x4 | 0x20 | 0x1, // DRAW_TO_WINDOW | SUPPORT_OPENGL | DOUBLEBUFFER
        iPixelType: 0,
        cColorBits: 32,
        cDepthBits: 24,
        cStencilBits: 8,
        ..Default::default()
    };
    let format = ChoosePixelFormat(dc, &pfd);
    if format == 0 || SetPixelFormat(dc, format, &pfd) == 0 {
        return Err("No suitable OpenGL pixel format is available on this display.".into());
    }
    // A legacy context is needed first to reach wglCreateContextAttribsARB.
    let legacy = wglCreateContext(dc);
    if legacy.is_null() || wglMakeCurrent(dc, legacy) == 0 {
        return Err("Could not create an OpenGL context. Please update the graphics driver.".into());
    }
    type CreateContextAttribs = unsafe extern "system" fn(HDC, HGLRC, *const i32) -> HGLRC;
    let name = CString::new("wglCreateContextAttribsARB").unwrap();
    let create = wglGetProcAddress(name.as_ptr());
    if (create as usize) < 4 {
        return Err("This graphics driver does not support OpenGL 3.3, which the game needs.".into());
    }
    let create: CreateContextAttribs = std::mem::transmute(create);
    let attribs = [0x2091, 3, 0x2092, 3, 0x9126, 0x0001, 0];
    let rc = create(dc, null_mut(), attribs.as_ptr());
    if rc.is_null() {
        return Err("Could not create an OpenGL 3.3 core context. Please update the graphics driver.".into());
    }
    wglMakeCurrent(null_mut(), null_mut());
    wglDeleteContext(legacy);
    if wglMakeCurrent(dc, rc) == 0 {
        return Err("Could not activate the OpenGL 3.3 context.".into());
    }
    let lib = CString::new("opengl32.dll").unwrap();
    let opengl32 = LoadLibraryA(lib.as_ptr());
    let loader = |symbol: &str| -> *const c_void {
        let c = CString::new(symbol).unwrap();
        let p = wglGetProcAddress(c.as_ptr());
        // Some drivers return small sentinel values instead of NULL for unknown names.
        if (p as usize) > 3 && p as isize != -1 {
            p
        } else if opengl32.is_null() {
            null()
        } else {
            GetProcAddress(opengl32, c.as_ptr())
        }
    };
    gl::init(&loader)?;
    // Vertical sync, when the driver offers it.
    let swap = loader("wglSwapIntervalEXT");
    if !swap.is_null() {
        let swap: unsafe extern "system" fn(i32) -> BOOL = std::mem::transmute(swap);
        swap(1);
    }
    ShowWindow(hwnd, 5);
    let _ = WS_VISIBLE;
    Ok(Window { hwnd, dc, fullscreen: false, saved: WINDOWPLACEMENT::default(), grabbed: false })
}

/// Runs the game until the window is closed.
pub fn run() -> Result<(), String> {
    std::panic::set_hook(Box::new(|info| {
        fatal(&format!("The game ran into a problem and has to close.\n\n{info}\n\nDetails were written to latest.log in the game data folder."));
    }));
    // SAFETY: called once on the main thread.
    let mut window = unsafe { create_window()? };
    let (w, h) = window.client_size();
    let mut game = Game::new(w, h);
    let mut events: Vec<Event> = Vec::new();
    loop {
        // SAFETY: standard message pump for windows owned by this thread.
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    request_quit();
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if quit_requested() {
            break;
        }
        // Cursor capture follows the game's wishes while the window is in the foreground.
        // SAFETY: trivial query.
        let focused = unsafe { GetForegroundWindow() } == window.hwnd;
        window.set_grab(cursor_grab_requested() && focused);
        window.poll_mouse();
        EVENTS.with(|q| events.append(&mut q.borrow_mut()));
        let mut fullscreen = false;
        for ev in events.drain(..) {
            match ev {
                Event::Key { code: key::F11, down: true } => fullscreen = true,
                Event::Focus(false) => {
                    super::native::set_cursor_grab(false);
                    game.handle(ev);
                }
                _ => game.handle(ev),
            }
        }
        if fullscreen {
            window.toggle_fullscreen();
        }
        game.frame(now_ms());
        // SAFETY: valid device context of our window.
        unsafe { SwapBuffers(window.dc) };
    }
    game.shutdown();
    window.set_grab(false);
    // SAFETY: the window belongs to this thread.
    unsafe { DestroyWindow(window.hwnd) };
    Ok(())
}
