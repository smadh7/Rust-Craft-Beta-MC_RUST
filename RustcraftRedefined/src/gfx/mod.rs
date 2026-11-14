//! Rendering: block texture array, chunk passes, sky, and the 2D/3D vertex batch.

pub mod batch;
pub mod chunks;
pub mod gl;

use crate::art::{blocktex, font};
use crate::util::math::{DVec3, Frustum, Mat4, Vec3};
use crate::util::rng::Rng;
use crate::world::block::reg;
use crate::world::World;
use batch::{rgba, Batch, Rgba, LAYER_ATLAS, LAYER_NONE};
use chunks::ChunkRenderer;

const CHUNK_VS: &str = "
in vec4 a_pos;
in vec2 a_uv;
in vec4 a_color;
in vec4 a_light;
uniform mat4 u_mvp;
uniform vec3 u_offset;
out vec3 v_uvl;
out vec4 v_color;
out vec2 v_light;
out float v_dist;
void main() {
    vec3 p = a_pos.xyz / 256.0 + u_offset;
    gl_Position = u_mvp * vec4(p, 1.0);
    v_uvl = vec3(a_uv / 4096.0, a_pos.w);
    v_color = a_color;
    v_light = a_light.xy;
    v_dist = length(p);
}";

const CHUNK_FS: &str = "
uniform sampler2DArray u_tex;
uniform vec3 u_fog_color;
uniform vec2 u_fog;
uniform vec3 u_sky_light;
uniform float u_alpha_test;
uniform float u_gamma;
in vec3 v_uvl;
in vec4 v_color;
in vec2 v_light;
in float v_dist;
out vec4 o_color;
void main() {
    vec4 t = texture(u_tex, v_uvl);
    if (t.a < u_alpha_test) discard;
    vec2 l = v_light / (4.0 - 3.0 * v_light);
    vec3 light = max(l.x * u_sky_light, l.y * vec3(1.0, 0.87, 0.70));
    light = mix(light, vec3(1.0), 0.035 + u_gamma * 0.2 * (1.0 - max(light.r, light.g)));
    vec3 c = t.rgb * v_color.rgb * light;
    float f = clamp((v_dist - u_fog.x) / (u_fog.y - u_fog.x), 0.0, 1.0);
    o_color = vec4(mix(c, u_fog_color, f), t.a);
}";

const BASIC_VS: &str = "
in vec3 a_pos;
in vec3 a_uvl;
in vec4 a_color;
uniform mat4 u_mvp;
out vec3 v_uvl;
out vec4 v_color;
out float v_dist;
void main() {
    gl_Position = u_mvp * vec4(a_pos, 1.0);
    v_uvl = a_uvl;
    v_color = a_color;
    v_dist = length(a_pos);
}";

const BASIC_FS: &str = "
uniform sampler2DArray u_tex;
uniform sampler2D u_atlas;
uniform vec3 u_fog_color;
uniform vec2 u_fog;
in vec3 v_uvl;
in vec4 v_color;
in float v_dist;
out vec4 o_color;
void main() {
    vec4 t = vec4(1.0);
    if (v_uvl.z >= 0.0) t = texture(u_tex, v_uvl);
    else if (v_uvl.z > -1.5) t = texture(u_atlas, v_uvl.xy);
    vec4 c = t * v_color;
    if (c.a < 0.004) discard;
    float f = u_fog.y > 0.0 ? clamp((v_dist - u_fog.x) / (u_fog.y - u_fog.x), 0.0, 1.0) : 0.0;
    o_color = vec4(mix(c.rgb, u_fog_color, f), c.a);
}";

/// Everything the renderer needs to know about the atmosphere for one frame.
#[derive(Clone, Copy, Debug)]
pub struct Environment {
    pub sky: [f32; 3],
    pub fog: [f32; 3],
    /// Colour and strength of sky light (white at noon, dim blue at night).
    pub sky_light: [f32; 3],
    /// 0 = noon, 0.25 = sunset, 0.5 = midnight, 0.75 = sunrise.
    pub celestial: f32,
    pub stars: f32,
    pub fog_start: f32,
    pub fog_end: f32,
    /// Strength of the warm glow around the sun at dawn and dusk.
    pub sunset: f32,
    pub gamma: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub pos: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
}

struct ChunkUniforms {
    mvp: i32,
    offset: i32,
    fog_color: i32,
    fog: i32,
    sky_light: i32,
    alpha_test: i32,
    gamma: i32,
}

pub struct Renderer {
    pub width: i32,
    pub height: i32,
    chunk_prog: u32,
    cu: ChunkUniforms,
    basic_prog: u32,
    basic_mvp: i32,
    basic_fog_color: i32,
    basic_fog: i32,
    block_tex: u32,
    atlas_tex: u32,
    pub batch: Batch,
    pub chunks: ChunkRenderer,
    pub font: font::Font,
    stars: Vec<(Vec3, f32)>,
    fluid_frame: u32,
    /// View-projection matrix of the last world pass (camera-relative).
    pub view_proj: Mat4,
}

fn f32rgb(c: [f32; 3], a: f32) -> Rgba {
    [(c[0].clamp(0.0, 1.0) * 255.0) as u8, (c[1].clamp(0.0, 1.0) * 255.0) as u8, (c[2].clamp(0.0, 1.0) * 255.0) as u8, (a.clamp(0.0, 1.0) * 255.0) as u8]
}

impl Renderer {
    pub fn new(width: i32, height: i32) -> Renderer {
        let head = gl::shader_header();
        let chunk_prog = gl::create_program(&format!("{head}{CHUNK_VS}"), &format!("{head}{CHUNK_FS}"), &["a_pos", "a_uv", "a_color", "a_light"]);
        let basic_prog = gl::create_program(&format!("{head}{BASIC_VS}"), &format!("{head}{BASIC_FS}"), &["a_pos", "a_uvl", "a_color"]);
        let u = |p: u32, n: &str| gl::uniform_location(p, n);
        let cu = ChunkUniforms {
            mvp: u(chunk_prog, "u_mvp"),
            offset: u(chunk_prog, "u_offset"),
            fog_color: u(chunk_prog, "u_fog_color"),
            fog: u(chunk_prog, "u_fog"),
            sky_light: u(chunk_prog, "u_sky_light"),
            alpha_test: u(chunk_prog, "u_alpha_test"),
            gamma: u(chunk_prog, "u_gamma"),
        };
        gl::use_program(chunk_prog);
        gl::uniform1i(u(chunk_prog, "u_tex"), 0);
        gl::use_program(basic_prog);
        gl::uniform1i(u(basic_prog, "u_tex"), 0);
        gl::uniform1i(u(basic_prog, "u_atlas"), 1);

        // Block textures: one array layer per texture, with hand-built mip levels.
        let names = &reg().textures;
        let block_tex = gl::create_texture();
        gl::active_texture(gl::TEXTURE0);
        gl::bind_texture(gl::TEXTURE_2D_ARRAY, block_tex);
        let mut levels: Vec<Vec<u8>> = vec![Vec::new(); 5];
        for name in names {
            let mut img = blocktex::generate(name).unwrap_or_else(blocktex::missing);
            for level in levels.iter_mut() {
                level.extend_from_slice(&img.bytes());
                img = img.half();
            }
        }
        for (i, data) in levels.iter().enumerate() {
            let size = 16 >> i;
            gl::tex_image_3d(gl::TEXTURE_2D_ARRAY, i as i32, gl::RGBA8, size, size, names.len() as i32, gl::RGBA, gl::UNSIGNED_BYTE, Some(data));
        }
        gl::tex_parameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_MAX_LEVEL, 4);
        gl::tex_parameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_MIN_FILTER, gl::NEAREST_MIPMAP_LINEAR);
        gl::tex_parameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_MAG_FILTER, gl::NEAREST);
        gl::tex_parameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_WRAP_S, gl::REPEAT);
        gl::tex_parameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_WRAP_T, gl::REPEAT);

        let atlas = font::build_atlas();
        let atlas_tex = gl::create_texture();
        gl::active_texture(gl::TEXTURE0 + 1);
        gl::bind_texture(gl::TEXTURE_2D, atlas_tex);
        gl::tex_image_2d(gl::TEXTURE_2D, 0, gl::RGBA8, atlas.w as i32, atlas.h as i32, gl::RGBA, gl::UNSIGNED_BYTE, Some(&atlas.bytes()));
        gl::tex_parameteri(gl::TEXTURE_2D, gl::TEXTURE_MAX_LEVEL, 0);
        gl::tex_parameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST);
        gl::tex_parameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST);
        gl::tex_parameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE);
        gl::tex_parameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE);
        gl::active_texture(gl::TEXTURE0);

        let mut rng = Rng::new(10842);
        let mut stars = Vec::new();
        while stars.len() < 900 {
            let v = Vec3::new(rng.f32() * 2.0 - 1.0, rng.f32() * 2.0 - 1.0, rng.f32() * 2.0 - 1.0);
            let l = v.length_sq();
            if l > 0.01 && l < 1.0 {
                stars.push((v.normalized(), 0.15 + rng.f32() * 0.25));
            }
        }
        Renderer {
            width,
            height,
            chunk_prog,
            cu,
            basic_prog,
            basic_mvp: u(basic_prog, "u_mvp"),
            basic_fog_color: u(basic_prog, "u_fog_color"),
            basic_fog: u(basic_prog, "u_fog"),
            block_tex,
            atlas_tex,
            batch: Batch::new(),
            chunks: ChunkRenderer::new(),
            font: font::Font::new(),
            stars,
            fluid_frame: u32::MAX,
            view_proj: Mat4::IDENTITY,
        }
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        self.width = width.max(1);
        self.height = height.max(1);
    }

    /// Advances animated textures; call with the running game tick.
    pub fn animate(&mut self, tick: u64) {
        let frame = (tick / 2) as u32 % blocktex::FLUID_FRAMES;
        if frame == self.fluid_frame {
            return;
        }
        self.fluid_frame = frame;
        gl::active_texture(gl::TEXTURE0);
        gl::bind_texture(gl::TEXTURE_2D_ARRAY, self.block_tex);
        for (layer, name) in ["water_still", "water_flow", "lava_still", "lava_flow"].iter().enumerate() {
            // Lava moves more slowly than water.
            let t = if layer >= 2 { (tick / 6) as u32 % blocktex::FLUID_FRAMES } else { frame };
            let Some(mut img) = blocktex::fluid(name, t) else { continue };
            for level in 0..5 {
                let size = 16 >> level;
                gl::tex_sub_image_3d(gl::TEXTURE_2D_ARRAY, level, 0, 0, layer as i32, size, size, 1, gl::RGBA, gl::UNSIGNED_BYTE, &img.bytes());
                img = img.half();
            }
        }
    }

    fn bind_textures(&self) {
        gl::active_texture(gl::TEXTURE0 + 1);
        gl::bind_texture(gl::TEXTURE_2D, self.atlas_tex);
        gl::active_texture(gl::TEXTURE0);
        gl::bind_texture(gl::TEXTURE_2D_ARRAY, self.block_tex);
    }

    fn use_basic(&self, mvp: &Mat4, fog: Option<(&Environment, f32)>) {
        gl::use_program(self.basic_prog);
        gl::uniform_matrix4(self.basic_mvp, &mvp.0);
        match fog {
            Some((env, scale)) => {
                gl::uniform3f(self.basic_fog_color, env.fog[0], env.fog[1], env.fog[2]);
                gl::uniform2f(self.basic_fog, env.fog_start * scale, env.fog_end * scale);
            }
            None => gl::uniform2f(self.basic_fog, 0.0, 0.0),
        }
    }

    fn draw_sky(&mut self, env: &Environment, view_proj: &Mat4) {
        gl::disable(gl::DEPTH_TEST);
        gl::disable(gl::CULL_FACE);
        gl::depth_mask(false);
        self.use_basic(view_proj, None);
        // Dome: zenith colour overhead blending to the fog colour at the horizon, dark void below.
        let rings: [(f32, [f32; 3]); 5] = [
            (90.0, env.sky),
            (28.0, [env.sky[0] * 0.75 + env.fog[0] * 0.25, env.sky[1] * 0.75 + env.fog[1] * 0.25, env.sky[2] * 0.75 + env.fog[2] * 0.25]),
            (6.0, env.fog),
            (-4.0, env.fog),
            (-90.0, [env.fog[0] * 0.35, env.fog[1] * 0.35, env.fog[2] * 0.4]),
        ];
        const SEG: usize = 24;
        let point = |elev: f32, i: usize| {
            let a = i as f32 / SEG as f32 * std::f32::consts::TAU;
            let (se, ce) = elev.to_radians().sin_cos();
            [a.cos() * ce * 50.0, se * 50.0, a.sin() * ce * 50.0]
        };
        for w in rings.windows(2) {
            let ((e0, c0), (e1, c1)) = (w[0], w[1]);
            for i in 0..SEG {
                let (a, b, c, d) = (point(e0, i), point(e0, i + 1), point(e1, i + 1), point(e1, i));
                let (k0, k1) = (f32rgb(c0, 1.0), f32rgb(c1, 1.0));
                self.batch.quad_colors([a, b, c, d], [[0.0; 2]; 4], LAYER_NONE, [k0, k0, k1, k1]);
            }
        }
        self.batch.flush(gl::TRIANGLES);

        gl::enable(gl::BLEND);
        let ang = env.celestial * std::f32::consts::TAU;
        let sun = Vec3::new(-ang.sin(), ang.cos(), 0.0);
        // Warm glow around the sun at dawn and dusk.
        if env.sunset > 0.01 {
            let side = Vec3::new(0.0, 0.0, 1.0);
            let base = Vec3::new(sun.x, 0.0, 0.0).normalized() * 45.0;
            let glow = rgba(0xFF8A3C, (env.sunset * 150.0) as u8);
            let none = rgba(0xFF8A3C, 0);
            let p = |v: Vec3| [v.x, v.y, v.z];
            for s in [-1.0f32, 1.0] {
                self.batch.quad_colors(
                    [p(base + Vec3::new(0.0, -6.0, 0.0)), p(base + side * (40.0 * s) + Vec3::new(0.0, -6.0, 0.0)), p(base * 0.6 + side * (40.0 * s) + Vec3::new(0.0, 22.0, 0.0)), p(base * 0.6 + Vec3::new(0.0, 22.0, 0.0))],
                    [[0.0; 2]; 4],
                    LAYER_NONE,
                    [glow, none, none, none],
                );
            }
            self.batch.flush(gl::TRIANGLES);
        }
        // Stars turn with the sky.
        if env.stars > 0.01 {
            let rot = Mat4::rotation_z(ang);
            let alpha = (env.stars * 255.0) as u8;
            for i in 0..self.stars.len() {
                let (dir, size) = self.stars[i];
                let d = rot.transform_dir(dir);
                let t1 = d.cross(Vec3::new(0.3, 1.0, 0.2)).normalized() * size;
                let t2 = d.cross(t1).normalized() * size;
                let c = d * 48.0;
                let p = |v: Vec3| [v.x, v.y, v.z];
                self.batch.quad([p(c - t1 - t2), p(c + t1 - t2), p(c + t1 + t2), p(c - t1 + t2)], [[0.0; 2]; 4], LAYER_NONE, [255, 255, 255, alpha]);
            }
            self.batch.flush(gl::TRIANGLES);
        }
        // Sun and moon.
        let s = 1.0 / font::ATLAS_SIZE as f32;
        for (dir, rect, size, alpha) in [(sun, font::R_SUN, 9.0, 1.0), (-sun, font::R_MOON, 6.5, 1.0)] {
            if dir.y < -0.25 {
                continue;
            }
            let t1 = Vec3::new(0.0, 0.0, size);
            let t2 = dir.cross(t1).normalized() * size;
            let c = dir * 45.0;
            let p = |v: Vec3| [v.x, v.y, v.z];
            let (u0, v0, u1, v1) = (rect[0] as f32 * s, rect[1] as f32 * s, (rect[0] + rect[2]) as f32 * s, (rect[1] + rect[3]) as f32 * s);
            self.batch.quad([p(c - t1 - t2), p(c + t1 - t2), p(c + t1 + t2), p(c - t1 + t2)], [[u0, v1], [u1, v1], [u1, v0], [u0, v0]], LAYER_ATLAS, [255, 255, 255, (alpha * 255.0) as u8]);
        }
        self.batch.flush(gl::TRIANGLES);
        gl::disable(gl::BLEND);
        gl::depth_mask(true);
        gl::enable(gl::DEPTH_TEST);
    }

    /// Draws sky and terrain.  Afterwards the basic program is active with the world matrix, so
    /// the caller can add outlines, particles and entities through `batch` and `flush_world`.
    pub fn draw_world(&mut self, world: &mut World, cam: &Camera, env: &Environment, render_distance: i32) {
        gl::viewport(0, 0, self.width, self.height);
        gl::clear_color(env.fog[0], env.fog[1], env.fog[2], 1.0);
        gl::clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        gl::depth_func(gl::LEQUAL);
        self.bind_textures();
        let far = (render_distance as f32 * 16.0 * 1.6).max(160.0);
        let proj = Mat4::perspective(cam.fov.to_radians(), self.width as f32 / self.height as f32, 0.05, far);
        let view = Mat4::view_rotation(cam.yaw, cam.pitch);
        let vp = proj.mul(&view);
        self.view_proj = vp;
        self.draw_sky(env, &vp);

        let frustum = Frustum::from_matrix(&vp);
        gl::use_program(self.chunk_prog);
        gl::uniform_matrix4(self.cu.mvp, &vp.0);
        gl::uniform3f(self.cu.fog_color, env.fog[0], env.fog[1], env.fog[2]);
        gl::uniform2f(self.cu.fog, env.fog_start, env.fog_end);
        gl::uniform3f(self.cu.sky_light, env.sky_light[0], env.sky_light[1], env.sky_light[2]);
        gl::uniform1f(self.cu.gamma, env.gamma);
        gl::uniform1f(self.cu.alpha_test, 0.5);
        gl::enable(gl::CULL_FACE);
        gl::cull_face(gl::BACK);
        gl::disable(gl::BLEND);
        self.chunks.draw(0, cam.pos, &frustum, render_distance, self.cu.offset);
        let _ = world;
        self.use_basic(&vp, Some((env, 1.0)));
    }

    /// Translucent terrain (water, ice, stained glass); call after opaque world geometry.
    pub fn draw_translucent(&mut self, cam: &Camera, env: &Environment, render_distance: i32) {
        let frustum = Frustum::from_matrix(&self.view_proj);
        gl::use_program(self.chunk_prog);
        gl::uniform1f(self.cu.alpha_test, 0.004);
        gl::enable(gl::BLEND);
        gl::blend_func(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        gl::enable(gl::CULL_FACE);
        self.chunks.draw(1, cam.pos, &frustum, render_distance, self.cu.offset);
        let vp = self.view_proj;
        self.use_basic(&vp, Some((env, 1.0)));
    }

    /// Flushes batched world-space geometry (positions relative to the camera).
    pub fn flush_world(&mut self, mode: u32, blend: bool) {
        if blend {
            gl::enable(gl::BLEND);
            gl::blend_func(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        } else {
            gl::disable(gl::BLEND);
        }
        self.batch.flush(mode);
    }

    /// Switches to 2D drawing in GUI units; returns the GUI size (width, height).
    pub fn begin_ui(&mut self, scale: f32) -> (f32, f32) {
        let (w, h) = (self.width as f32 / scale, self.height as f32 / scale);
        gl::disable(gl::DEPTH_TEST);
        gl::disable(gl::CULL_FACE);
        gl::enable(gl::BLEND);
        gl::blend_func(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        self.bind_textures();
        let ortho = Mat4::ortho(0.0, w, h, 0.0, -100.0, 100.0);
        self.use_basic(&ortho, None);
        (w, h)
    }

    pub fn end_ui(&mut self) {
        self.batch.flush(gl::TRIANGLES);
        gl::disable(gl::BLEND);
        gl::enable(gl::DEPTH_TEST);
    }

    /// Reads the framebuffer as top-down RGBA rows.
    pub fn capture(&self) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut px = vec![0u8; w * h * 4];
        gl::read_pixels(0, 0, self.width, self.height, &mut px);
        let mut out = vec![0u8; w * h * 4];
        for y in 0..h {
            out[y * w * 4..(y + 1) * w * 4].copy_from_slice(&px[(h - 1 - y) * w * 4..(h - y) * w * 4]);
        }
        for p in out.chunks_mut(4) {
            p[3] = 255;
        }
        out
    }
}
