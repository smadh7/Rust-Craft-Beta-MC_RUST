//! A thin graphics API over the common subset of OpenGL 3.3 core and WebGL 2.
//! Native builds call OpenGL through function pointers loaded at start-up; the web build calls
//! JavaScript imports that forward to WebGL.  Object handles are plain integers on both.
#![allow(clippy::too_many_arguments)]

pub const DEPTH_BUFFER_BIT: u32 = 0x0100;
pub const COLOR_BUFFER_BIT: u32 = 0x4000;
pub const LINES: u32 = 0x0001;
pub const TRIANGLES: u32 = 0x0004;
pub const LEQUAL: u32 = 0x0203;
pub const ALWAYS: u32 = 0x0207;
pub const SRC_ALPHA: u32 = 0x0302;
pub const ONE_MINUS_SRC_ALPHA: u32 = 0x0303;
pub const ONE: u32 = 1;
pub const ZERO: u32 = 0;
pub const BACK: u32 = 0x0405;
pub const CULL_FACE: u32 = 0x0B44;
pub const DEPTH_TEST: u32 = 0x0B71;
pub const BLEND: u32 = 0x0BE2;
pub const POLYGON_OFFSET_FILL: u32 = 0x8037;
pub const TEXTURE_2D: u32 = 0x0DE1;
pub const TEXTURE_2D_ARRAY: u32 = 0x8C1A;
pub const UNSIGNED_BYTE: u32 = 0x1401;
pub const SHORT: u32 = 0x1402;
pub const UNSIGNED_INT: u32 = 0x1405;
pub const FLOAT: u32 = 0x1406;
pub const RGBA: u32 = 0x1908;
pub const RGBA8: u32 = 0x8058;
pub const NEAREST: i32 = 0x2600;
pub const LINEAR: i32 = 0x2601;
pub const NEAREST_MIPMAP_LINEAR: i32 = 0x2702;
pub const TEXTURE_MAG_FILTER: u32 = 0x2800;
pub const TEXTURE_MIN_FILTER: u32 = 0x2801;
pub const TEXTURE_WRAP_S: u32 = 0x2802;
pub const TEXTURE_WRAP_T: u32 = 0x2803;
pub const TEXTURE_MAX_LEVEL: u32 = 0x813D;
pub const REPEAT: i32 = 0x2901;
pub const CLAMP_TO_EDGE: i32 = 0x812F;
pub const TEXTURE0: u32 = 0x84C0;
pub const ARRAY_BUFFER: u32 = 0x8892;
pub const ELEMENT_ARRAY_BUFFER: u32 = 0x8893;
pub const STATIC_DRAW: u32 = 0x88E4;
pub const DYNAMIC_DRAW: u32 = 0x88E8;
pub const STREAM_DRAW: u32 = 0x88E0;

pub use imp::*;

/// Source prefix that makes one GLSL body valid for both back ends.
pub fn shader_header() -> &'static str {
    if cfg!(target_arch = "wasm32") {
        "#version 300 es\nprecision highp float;\nprecision highp int;\nprecision mediump sampler2DArray;\n"
    } else {
        "#version 330 core\n"
    }
}

// ------------------------------------------------------------------ WebGL 2
#[cfg(target_arch = "wasm32")]
mod imp {
    #[link(wasm_import_module = "gl")]
    extern "C" {
        #[link_name = "viewport"]
        fn js_viewport(x: i32, y: i32, w: i32, h: i32);
        #[link_name = "clear_color"]
        fn js_clear_color(r: f32, g: f32, b: f32, a: f32);
        #[link_name = "clear"]
        fn js_clear(mask: u32);
        #[link_name = "enable"]
        fn js_enable(cap: u32);
        #[link_name = "disable"]
        fn js_disable(cap: u32);
        #[link_name = "blend_func"]
        fn js_blend_func(s: u32, d: u32);
        #[link_name = "depth_func"]
        fn js_depth_func(f: u32);
        #[link_name = "depth_mask"]
        fn js_depth_mask(on: u32);
        #[link_name = "cull_face"]
        fn js_cull_face(mode: u32);
        #[link_name = "polygon_offset"]
        fn js_polygon_offset(factor: f32, units: f32);
        #[link_name = "create_program"]
        fn js_create_program(vs: *const u8, vs_len: usize, fs: *const u8, fs_len: usize, attribs: *const u8, attribs_len: usize) -> u32;
        #[link_name = "use_program"]
        fn js_use_program(p: u32);
        #[link_name = "uniform_location"]
        fn js_uniform_location(p: u32, name: *const u8, len: usize) -> i32;
        #[link_name = "uniform1i"]
        fn js_uniform1i(loc: i32, v: i32);
        #[link_name = "uniform1f"]
        fn js_uniform1f(loc: i32, v: f32);
        #[link_name = "uniform2f"]
        fn js_uniform2f(loc: i32, a: f32, b: f32);
        #[link_name = "uniform3f"]
        fn js_uniform3f(loc: i32, a: f32, b: f32, c: f32);
        #[link_name = "uniform4f"]
        fn js_uniform4f(loc: i32, a: f32, b: f32, c: f32, d: f32);
        #[link_name = "uniform_matrix4"]
        fn js_uniform_matrix4(loc: i32, m: *const f32);
        #[link_name = "create_buffer"]
        fn js_create_buffer() -> u32;
        #[link_name = "bind_buffer"]
        fn js_bind_buffer(target: u32, b: u32);
        #[link_name = "buffer_data"]
        fn js_buffer_data(target: u32, data: *const u8, len: usize, usage: u32);
        #[link_name = "buffer_sub_data"]
        fn js_buffer_sub_data(target: u32, offset: usize, data: *const u8, len: usize);
        #[link_name = "delete_buffer"]
        fn js_delete_buffer(b: u32);
        #[link_name = "create_vertex_array"]
        fn js_create_vertex_array() -> u32;
        #[link_name = "bind_vertex_array"]
        fn js_bind_vertex_array(v: u32);
        #[link_name = "delete_vertex_array"]
        fn js_delete_vertex_array(v: u32);
        #[link_name = "enable_vertex_attrib_array"]
        fn js_enable_vertex_attrib_array(i: u32);
        #[link_name = "vertex_attrib_pointer"]
        fn js_vertex_attrib_pointer(i: u32, size: i32, ty: u32, normalized: u32, stride: i32, offset: usize);
        #[link_name = "create_texture"]
        fn js_create_texture() -> u32;
        #[link_name = "bind_texture"]
        fn js_bind_texture(target: u32, t: u32);
        #[link_name = "active_texture"]
        fn js_active_texture(unit: u32);
        #[link_name = "delete_texture"]
        fn js_delete_texture(t: u32);
        #[link_name = "tex_parameteri"]
        fn js_tex_parameteri(target: u32, pname: u32, v: i32);
        #[link_name = "tex_image_2d"]
        fn js_tex_image_2d(target: u32, level: i32, internal: u32, w: i32, h: i32, format: u32, ty: u32, data: *const u8, len: usize);
        #[link_name = "tex_sub_image_2d"]
        fn js_tex_sub_image_2d(target: u32, level: i32, x: i32, y: i32, w: i32, h: i32, format: u32, ty: u32, data: *const u8, len: usize);
        #[link_name = "tex_image_3d"]
        fn js_tex_image_3d(target: u32, level: i32, internal: u32, w: i32, h: i32, d: i32, format: u32, ty: u32, data: *const u8, len: usize);
        #[link_name = "tex_sub_image_3d"]
        fn js_tex_sub_image_3d(target: u32, level: i32, x: i32, y: i32, z: i32, w: i32, h: i32, d: i32, format: u32, ty: u32, data: *const u8, len: usize);
        #[link_name = "draw_arrays"]
        fn js_draw_arrays(mode: u32, first: i32, count: i32);
        #[link_name = "draw_elements"]
        fn js_draw_elements(mode: u32, count: i32, ty: u32, offset: usize);
        #[link_name = "read_pixels"]
        fn js_read_pixels(x: i32, y: i32, w: i32, h: i32, out: *mut u8, len: usize);
    }

    // SAFETY (whole module): every call forwards plain values or pointer/length pairs that stay
    // valid for the duration of the synchronous JavaScript call.
    pub fn viewport(x: i32, y: i32, w: i32, h: i32) { unsafe { js_viewport(x, y, w, h) } }
    pub fn clear_color(r: f32, g: f32, b: f32, a: f32) { unsafe { js_clear_color(r, g, b, a) } }
    pub fn clear(mask: u32) { unsafe { js_clear(mask) } }
    pub fn enable(cap: u32) { unsafe { js_enable(cap) } }
    pub fn disable(cap: u32) { unsafe { js_disable(cap) } }
    pub fn blend_func(s: u32, d: u32) { unsafe { js_blend_func(s, d) } }
    pub fn depth_func(f: u32) { unsafe { js_depth_func(f) } }
    pub fn depth_mask(on: bool) { unsafe { js_depth_mask(on as u32) } }
    pub fn cull_face(mode: u32) { unsafe { js_cull_face(mode) } }
    pub fn polygon_offset(factor: f32, units: f32) { unsafe { js_polygon_offset(factor, units) } }
    pub fn create_program(vs: &str, fs: &str, attribs: &[&str]) -> u32 {
        let list = attribs.join(",");
        unsafe { js_create_program(vs.as_ptr(), vs.len(), fs.as_ptr(), fs.len(), list.as_ptr(), list.len()) }
    }
    pub fn use_program(p: u32) { unsafe { js_use_program(p) } }
    pub fn uniform_location(p: u32, name: &str) -> i32 { unsafe { js_uniform_location(p, name.as_ptr(), name.len()) } }
    pub fn uniform1i(loc: i32, v: i32) { unsafe { js_uniform1i(loc, v) } }
    pub fn uniform1f(loc: i32, v: f32) { unsafe { js_uniform1f(loc, v) } }
    pub fn uniform2f(loc: i32, a: f32, b: f32) { unsafe { js_uniform2f(loc, a, b) } }
    pub fn uniform3f(loc: i32, a: f32, b: f32, c: f32) { unsafe { js_uniform3f(loc, a, b, c) } }
    pub fn uniform4f(loc: i32, a: f32, b: f32, c: f32, d: f32) { unsafe { js_uniform4f(loc, a, b, c, d) } }
    pub fn uniform_matrix4(loc: i32, m: &[f32; 16]) { unsafe { js_uniform_matrix4(loc, m.as_ptr()) } }
    pub fn create_buffer() -> u32 { unsafe { js_create_buffer() } }
    pub fn bind_buffer(target: u32, b: u32) { unsafe { js_bind_buffer(target, b) } }
    pub fn buffer_data(target: u32, data: &[u8], usage: u32) { unsafe { js_buffer_data(target, data.as_ptr(), data.len(), usage) } }
    pub fn buffer_sub_data(target: u32, offset: usize, data: &[u8]) { unsafe { js_buffer_sub_data(target, offset, data.as_ptr(), data.len()) } }
    pub fn delete_buffer(b: u32) { unsafe { js_delete_buffer(b) } }
    pub fn create_vertex_array() -> u32 { unsafe { js_create_vertex_array() } }
    pub fn bind_vertex_array(v: u32) { unsafe { js_bind_vertex_array(v) } }
    pub fn delete_vertex_array(v: u32) { unsafe { js_delete_vertex_array(v) } }
    pub fn enable_vertex_attrib_array(i: u32) { unsafe { js_enable_vertex_attrib_array(i) } }
    pub fn vertex_attrib_pointer(i: u32, size: i32, ty: u32, normalized: bool, stride: i32, offset: usize) { unsafe { js_vertex_attrib_pointer(i, size, ty, normalized as u32, stride, offset) } }
    pub fn create_texture() -> u32 { unsafe { js_create_texture() } }
    pub fn bind_texture(target: u32, t: u32) { unsafe { js_bind_texture(target, t) } }
    pub fn active_texture(unit: u32) { unsafe { js_active_texture(unit) } }
    pub fn delete_texture(t: u32) { unsafe { js_delete_texture(t) } }
    pub fn tex_parameteri(target: u32, pname: u32, v: i32) { unsafe { js_tex_parameteri(target, pname, v) } }
    pub fn tex_image_2d(target: u32, level: i32, internal: u32, w: i32, h: i32, format: u32, ty: u32, data: Option<&[u8]>) {
        let (p, l) = data.map_or((std::ptr::null(), 0), |d| (d.as_ptr(), d.len()));
        unsafe { js_tex_image_2d(target, level, internal, w, h, format, ty, p, l) }
    }
    pub fn tex_sub_image_2d(target: u32, level: i32, x: i32, y: i32, w: i32, h: i32, format: u32, ty: u32, data: &[u8]) {
        unsafe { js_tex_sub_image_2d(target, level, x, y, w, h, format, ty, data.as_ptr(), data.len()) }
    }
    pub fn tex_image_3d(target: u32, level: i32, internal: u32, w: i32, h: i32, d: i32, format: u32, ty: u32, data: Option<&[u8]>) {
        let (p, l) = data.map_or((std::ptr::null(), 0), |d| (d.as_ptr(), d.len()));
        unsafe { js_tex_image_3d(target, level, internal, w, h, d, format, ty, p, l) }
    }
    pub fn tex_sub_image_3d(target: u32, level: i32, x: i32, y: i32, z: i32, w: i32, h: i32, d: i32, format: u32, ty: u32, data: &[u8]) {
        unsafe { js_tex_sub_image_3d(target, level, x, y, z, w, h, d, format, ty, data.as_ptr(), data.len()) }
    }
    pub fn draw_arrays(mode: u32, first: i32, count: i32) { unsafe { js_draw_arrays(mode, first, count) } }
    pub fn draw_elements(mode: u32, count: i32, ty: u32, offset: usize) { unsafe { js_draw_elements(mode, count, ty, offset) } }
    pub fn read_pixels(x: i32, y: i32, w: i32, h: i32, out: &mut [u8]) { unsafe { js_read_pixels(x, y, w, h, out.as_mut_ptr(), out.len()) } }
}

// ------------------------------------------------------------------ OpenGL 3.3 core
#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::ffi::{c_char, c_void, CString};
    use std::sync::OnceLock;

    macro_rules! gl_functions {
        ($($field:ident = $name:literal : fn($($arg:ty),*) $(-> $ret:ty)?;)*) => {
            #[allow(non_snake_case)]
            struct Api { $($field: unsafe extern "system" fn($($arg),*) $(-> $ret)?),* }
            impl Api {
                /// Resolves every entry point; returns the name of the first missing one.
                fn load(loader: &dyn Fn(&str) -> *const c_void) -> Result<Api, &'static str> {
                    Ok(Api { $($field: {
                        let p = loader($name);
                        if p.is_null() { return Err($name); }
                        // SAFETY: the pointer comes from the GL loader for this exact entry point.
                        unsafe { std::mem::transmute::<*const c_void, unsafe extern "system" fn($($arg),*) $(-> $ret)?>(p) }
                    }),* })
                }
            }
        };
    }
    gl_functions! {
        viewport = "glViewport": fn(i32, i32, i32, i32);
        clear_color = "glClearColor": fn(f32, f32, f32, f32);
        clear = "glClear": fn(u32);
        enable = "glEnable": fn(u32);
        disable = "glDisable": fn(u32);
        blend_func = "glBlendFunc": fn(u32, u32);
        depth_func = "glDepthFunc": fn(u32);
        depth_mask = "glDepthMask": fn(u8);
        cull_face = "glCullFace": fn(u32);
        polygon_offset = "glPolygonOffset": fn(f32, f32);
        create_shader = "glCreateShader": fn(u32) -> u32;
        shader_source = "glShaderSource": fn(u32, i32, *const *const c_char, *const i32);
        compile_shader = "glCompileShader": fn(u32);
        get_shaderiv = "glGetShaderiv": fn(u32, u32, *mut i32);
        get_shader_info_log = "glGetShaderInfoLog": fn(u32, i32, *mut i32, *mut c_char);
        delete_shader = "glDeleteShader": fn(u32);
        create_program = "glCreateProgram": fn() -> u32;
        attach_shader = "glAttachShader": fn(u32, u32);
        bind_attrib_location = "glBindAttribLocation": fn(u32, u32, *const c_char);
        link_program = "glLinkProgram": fn(u32);
        get_programiv = "glGetProgramiv": fn(u32, u32, *mut i32);
        get_program_info_log = "glGetProgramInfoLog": fn(u32, i32, *mut i32, *mut c_char);
        use_program = "glUseProgram": fn(u32);
        get_uniform_location = "glGetUniformLocation": fn(u32, *const c_char) -> i32;
        uniform1i = "glUniform1i": fn(i32, i32);
        uniform1f = "glUniform1f": fn(i32, f32);
        uniform2f = "glUniform2f": fn(i32, f32, f32);
        uniform3f = "glUniform3f": fn(i32, f32, f32, f32);
        uniform4f = "glUniform4f": fn(i32, f32, f32, f32, f32);
        uniform_matrix4fv = "glUniformMatrix4fv": fn(i32, i32, u8, *const f32);
        gen_buffers = "glGenBuffers": fn(i32, *mut u32);
        bind_buffer = "glBindBuffer": fn(u32, u32);
        buffer_data = "glBufferData": fn(u32, isize, *const c_void, u32);
        buffer_sub_data = "glBufferSubData": fn(u32, isize, isize, *const c_void);
        delete_buffers = "glDeleteBuffers": fn(i32, *const u32);
        gen_vertex_arrays = "glGenVertexArrays": fn(i32, *mut u32);
        bind_vertex_array = "glBindVertexArray": fn(u32);
        delete_vertex_arrays = "glDeleteVertexArrays": fn(i32, *const u32);
        enable_vertex_attrib_array = "glEnableVertexAttribArray": fn(u32);
        vertex_attrib_pointer = "glVertexAttribPointer": fn(u32, i32, u32, u8, i32, *const c_void);
        gen_textures = "glGenTextures": fn(i32, *mut u32);
        bind_texture = "glBindTexture": fn(u32, u32);
        active_texture = "glActiveTexture": fn(u32);
        delete_textures = "glDeleteTextures": fn(i32, *const u32);
        tex_parameteri = "glTexParameteri": fn(u32, u32, i32);
        tex_image_2d = "glTexImage2D": fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void);
        tex_sub_image_2d = "glTexSubImage2D": fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void);
        tex_image_3d = "glTexImage3D": fn(u32, i32, i32, i32, i32, i32, i32, u32, u32, *const c_void);
        tex_sub_image_3d = "glTexSubImage3D": fn(u32, i32, i32, i32, i32, i32, i32, i32, u32, u32, *const c_void);
        draw_arrays = "glDrawArrays": fn(u32, i32, i32);
        draw_elements = "glDrawElements": fn(u32, i32, u32, *const c_void);
        read_pixels = "glReadPixels": fn(i32, i32, i32, i32, u32, u32, *mut c_void);
        pixel_storei = "glPixelStorei": fn(u32, i32);
    }

    static API: OnceLock<Api> = OnceLock::new();

    /// Loads the OpenGL entry points.  Must be called once with a current 3.3 core context.
    pub fn init(loader: &dyn Fn(&str) -> *const c_void) -> Result<(), String> {
        let api = Api::load(loader).map_err(|name| format!("OpenGL function {name} is not available (OpenGL 3.3 is required)"))?;
        // SAFETY: a context is current; pack/unpack alignment 1 matches tightly packed buffers.
        unsafe {
            (api.pixel_storei)(0x0CF5, 1);
            (api.pixel_storei)(0x0D05, 1);
        }
        let _ = API.set(api);
        Ok(())
    }

    #[inline]
    fn api() -> &'static Api {
        API.get().expect("OpenGL used before gl::init")
    }

    fn compile(kind: u32, src: &str) -> Result<u32, String> {
        let a = api();
        // SAFETY: valid shader object and NUL-free source passed with an explicit length.
        unsafe {
            let s = (a.create_shader)(kind);
            let ptr = src.as_ptr() as *const c_char;
            let len = src.len() as i32;
            (a.shader_source)(s, 1, &ptr, &len);
            (a.compile_shader)(s);
            let mut ok = 0;
            (a.get_shaderiv)(s, 0x8B81, &mut ok);
            if ok == 0 {
                let mut buf = vec![0u8; 2048];
                let mut n = 0;
                (a.get_shader_info_log)(s, 2048, &mut n, buf.as_mut_ptr() as *mut c_char);
                return Err(String::from_utf8_lossy(&buf[..n.max(0) as usize]).into_owned());
            }
            Ok(s)
        }
    }

    // SAFETY (wrappers below): arguments are forwarded unchanged to the driver; slices outlive the call.
    pub fn viewport(x: i32, y: i32, w: i32, h: i32) { unsafe { (api().viewport)(x, y, w, h) } }
    pub fn clear_color(r: f32, g: f32, b: f32, a: f32) { unsafe { (api().clear_color)(r, g, b, a) } }
    pub fn clear(mask: u32) { unsafe { (api().clear)(mask) } }
    pub fn enable(cap: u32) { unsafe { (api().enable)(cap) } }
    pub fn disable(cap: u32) { unsafe { (api().disable)(cap) } }
    pub fn blend_func(s: u32, d: u32) { unsafe { (api().blend_func)(s, d) } }
    pub fn depth_func(f: u32) { unsafe { (api().depth_func)(f) } }
    pub fn depth_mask(on: bool) { unsafe { (api().depth_mask)(on as u8) } }
    pub fn cull_face(mode: u32) { unsafe { (api().cull_face)(mode) } }
    pub fn polygon_offset(factor: f32, units: f32) { unsafe { (api().polygon_offset)(factor, units) } }
    pub fn create_program(vs: &str, fs: &str, attribs: &[&str]) -> u32 {
        let a = api();
        let v = compile(0x8B31, vs).unwrap_or_else(|e| panic!("vertex shader failed to compile:\n{e}"));
        let f = compile(0x8B30, fs).unwrap_or_else(|e| panic!("fragment shader failed to compile:\n{e}"));
        unsafe {
            let p = (a.create_program)();
            (a.attach_shader)(p, v);
            (a.attach_shader)(p, f);
            for (i, name) in attribs.iter().enumerate() {
                let c = CString::new(*name).unwrap();
                (a.bind_attrib_location)(p, i as u32, c.as_ptr());
            }
            (a.link_program)(p);
            let mut ok = 0;
            (a.get_programiv)(p, 0x8B82, &mut ok);
            if ok == 0 {
                let mut buf = vec![0u8; 2048];
                let mut n = 0;
                (a.get_program_info_log)(p, 2048, &mut n, buf.as_mut_ptr() as *mut c_char);
                panic!("shader program failed to link:\n{}", String::from_utf8_lossy(&buf[..n.max(0) as usize]));
            }
            (a.delete_shader)(v);
            (a.delete_shader)(f);
            p
        }
    }
    pub fn use_program(p: u32) { unsafe { (api().use_program)(p) } }
    pub fn uniform_location(p: u32, name: &str) -> i32 {
        let c = CString::new(name).unwrap();
        unsafe { (api().get_uniform_location)(p, c.as_ptr()) }
    }
    pub fn uniform1i(loc: i32, v: i32) { unsafe { (api().uniform1i)(loc, v) } }
    pub fn uniform1f(loc: i32, v: f32) { unsafe { (api().uniform1f)(loc, v) } }
    pub fn uniform2f(loc: i32, a: f32, b: f32) { unsafe { (api().uniform2f)(loc, a, b) } }
    pub fn uniform3f(loc: i32, a: f32, b: f32, c: f32) { unsafe { (api().uniform3f)(loc, a, b, c) } }
    pub fn uniform4f(loc: i32, a: f32, b: f32, c: f32, d: f32) { unsafe { (api().uniform4f)(loc, a, b, c, d) } }
    pub fn uniform_matrix4(loc: i32, m: &[f32; 16]) { unsafe { (api().uniform_matrix4fv)(loc, 1, 0, m.as_ptr()) } }
    pub fn create_buffer() -> u32 { let mut b = 0; unsafe { (api().gen_buffers)(1, &mut b) }; b }
    pub fn bind_buffer(target: u32, b: u32) { unsafe { (api().bind_buffer)(target, b) } }
    pub fn buffer_data(target: u32, data: &[u8], usage: u32) { unsafe { (api().buffer_data)(target, data.len() as isize, data.as_ptr() as *const c_void, usage) } }
    pub fn buffer_sub_data(target: u32, offset: usize, data: &[u8]) { unsafe { (api().buffer_sub_data)(target, offset as isize, data.len() as isize, data.as_ptr() as *const c_void) } }
    pub fn delete_buffer(b: u32) { unsafe { (api().delete_buffers)(1, &b) } }
    pub fn create_vertex_array() -> u32 { let mut v = 0; unsafe { (api().gen_vertex_arrays)(1, &mut v) }; v }
    pub fn bind_vertex_array(v: u32) { unsafe { (api().bind_vertex_array)(v) } }
    pub fn delete_vertex_array(v: u32) { unsafe { (api().delete_vertex_arrays)(1, &v) } }
    pub fn enable_vertex_attrib_array(i: u32) { unsafe { (api().enable_vertex_attrib_array)(i) } }
    pub fn vertex_attrib_pointer(i: u32, size: i32, ty: u32, normalized: bool, stride: i32, offset: usize) { unsafe { (api().vertex_attrib_pointer)(i, size, ty, normalized as u8, stride, offset as *const c_void) } }
    pub fn create_texture() -> u32 { let mut t = 0; unsafe { (api().gen_textures)(1, &mut t) }; t }
    pub fn bind_texture(target: u32, t: u32) { unsafe { (api().bind_texture)(target, t) } }
    pub fn active_texture(unit: u32) { unsafe { (api().active_texture)(unit) } }
    pub fn delete_texture(t: u32) { unsafe { (api().delete_textures)(1, &t) } }
    pub fn tex_parameteri(target: u32, pname: u32, v: i32) { unsafe { (api().tex_parameteri)(target, pname, v) } }
    fn ptr(data: Option<&[u8]>) -> *const c_void { data.map_or(std::ptr::null(), |d| d.as_ptr() as *const c_void) }
    pub fn tex_image_2d(target: u32, level: i32, internal: u32, w: i32, h: i32, format: u32, ty: u32, data: Option<&[u8]>) {
        unsafe { (api().tex_image_2d)(target, level, internal as i32, w, h, 0, format, ty, ptr(data)) }
    }
    pub fn tex_sub_image_2d(target: u32, level: i32, x: i32, y: i32, w: i32, h: i32, format: u32, ty: u32, data: &[u8]) {
        unsafe { (api().tex_sub_image_2d)(target, level, x, y, w, h, format, ty, data.as_ptr() as *const c_void) }
    }
    pub fn tex_image_3d(target: u32, level: i32, internal: u32, w: i32, h: i32, d: i32, format: u32, ty: u32, data: Option<&[u8]>) {
        unsafe { (api().tex_image_3d)(target, level, internal as i32, w, h, d, 0, format, ty, ptr(data)) }
    }
    pub fn tex_sub_image_3d(target: u32, level: i32, x: i32, y: i32, z: i32, w: i32, h: i32, d: i32, format: u32, ty: u32, data: &[u8]) {
        unsafe { (api().tex_sub_image_3d)(target, level, x, y, z, w, h, d, format, ty, data.as_ptr() as *const c_void) }
    }
    pub fn draw_arrays(mode: u32, first: i32, count: i32) { unsafe { (api().draw_arrays)(mode, first, count) } }
    pub fn draw_elements(mode: u32, count: i32, ty: u32, offset: usize) { unsafe { (api().draw_elements)(mode, count, ty, offset as *const c_void) } }
    pub fn read_pixels(x: i32, y: i32, w: i32, h: i32, out: &mut [u8]) {
        assert!(out.len() >= (w * h * 4) as usize);
        unsafe { (api().read_pixels)(x, y, w, h, super::RGBA, super::UNSIGNED_BYTE, out.as_mut_ptr() as *mut c_void) }
    }
}
