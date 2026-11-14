//! Immediate-style vertex batch used for the GUI, outlines, sky sprites, particles and entities.

use super::gl;
use crate::art::font::{Font, Rect, ATLAS_SIZE, GLYPH_CELL, R_WHITE};
use crate::util::as_bytes;
use crate::util::math::Vec3;

/// Texture selector stored in the `layer` field.
pub const LAYER_ATLAS: f32 = -1.0;
pub const LAYER_NONE: f32 = -2.0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    /// >= 0: block texture layer, -1: GUI atlas, -2: untextured.
    pub layer: f32,
    pub color: [u8; 4],
}

pub type Rgba = [u8; 4];
pub const WHITE: Rgba = [255, 255, 255, 255];

pub fn rgba(hex: u32, alpha: u8) -> Rgba {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, alpha]
}

pub struct Batch {
    pub verts: Vec<Vertex>,
    vao: u32,
    vbo: u32,
    capacity: usize,
}

impl Batch {
    pub fn new() -> Batch {
        let vao = gl::create_vertex_array();
        let vbo = gl::create_buffer();
        gl::bind_vertex_array(vao);
        gl::bind_buffer(gl::ARRAY_BUFFER, vbo);
        for i in 0..3 {
            gl::enable_vertex_attrib_array(i);
        }
        gl::vertex_attrib_pointer(0, 3, gl::FLOAT, false, 28, 0);
        gl::vertex_attrib_pointer(1, 3, gl::FLOAT, false, 28, 12);
        gl::vertex_attrib_pointer(2, 4, gl::UNSIGNED_BYTE, true, 28, 24);
        gl::bind_vertex_array(0);
        Batch { verts: Vec::with_capacity(4096), vao, vbo, capacity: 0 }
    }

    /// Uploads and draws everything queued so far with the given primitive mode.
    pub fn flush(&mut self, mode: u32) {
        if self.verts.is_empty() {
            return;
        }
        gl::bind_vertex_array(self.vao);
        gl::bind_buffer(gl::ARRAY_BUFFER, self.vbo);
        let bytes = as_bytes(&self.verts);
        if self.verts.len() > self.capacity {
            self.capacity = self.verts.len().next_power_of_two();
            let mut padded = bytes.to_vec();
            padded.resize(self.capacity * 28, 0);
            gl::buffer_data(gl::ARRAY_BUFFER, &padded, gl::STREAM_DRAW);
        } else {
            gl::buffer_sub_data(gl::ARRAY_BUFFER, 0, bytes);
        }
        gl::draw_arrays(mode, 0, self.verts.len() as i32);
        gl::bind_vertex_array(0);
        self.verts.clear();
    }

    #[inline]
    pub fn vertex(&mut self, pos: [f32; 3], uv: [f32; 2], layer: f32, color: Rgba) {
        self.verts.push(Vertex { pos, uv, layer, color });
    }

    /// Quad from four corners in counter-clockwise order with matching texture coordinates.
    pub fn quad(&mut self, p: [[f32; 3]; 4], uv: [[f32; 2]; 4], layer: f32, color: Rgba) {
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertex(p[i], uv[i], layer, color);
        }
    }

    /// Like `quad` with one colour per corner.
    pub fn quad_colors(&mut self, p: [[f32; 3]; 4], uv: [[f32; 2]; 4], layer: f32, colors: [Rgba; 4]) {
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertex(p[i], uv[i], layer, colors[i]);
        }
    }

    pub fn line(&mut self, a: Vec3, b: Vec3, color: Rgba) {
        self.vertex([a.x, a.y, a.z], [0.0; 2], LAYER_NONE, color);
        self.vertex([b.x, b.y, b.z], [0.0; 2], LAYER_NONE, color);
    }

    // ------------------------------------------------------------ 2D helpers (GUI pixels)
    /// Solid rectangle.
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
        self.sprite(x, y, w, h, R_WHITE, color);
    }

    /// Part of the GUI atlas stretched over a rectangle.
    pub fn sprite(&mut self, x: f32, y: f32, w: f32, h: f32, r: Rect, color: Rgba) {
        let s = 1.0 / ATLAS_SIZE as f32;
        let (u0, v0, u1, v1) = (r[0] as f32 * s, r[1] as f32 * s, (r[0] + r[2]) as f32 * s, (r[1] + r[3]) as f32 * s);
        self.quad([[x, y + h, 0.0], [x + w, y + h, 0.0], [x + w, y, 0.0], [x, y, 0.0]], [[u0, v1], [u1, v1], [u1, v0], [u0, v0]], LAYER_ATLAS, color);
    }

    /// Atlas sprite drawn at its natural size.
    pub fn icon(&mut self, x: f32, y: f32, r: Rect, color: Rgba) {
        self.sprite(x, y, r[2] as f32, r[3] as f32, r, color);
    }

    /// Stretchable panel: the corners keep their size, edges and centre are stretched.
    pub fn nine_slice(&mut self, x: f32, y: f32, w: f32, h: f32, r: Rect, border: u16, color: Rgba) {
        let b = border as f32;
        let xs = [x, x + b, x + w - b, x + w];
        let ys = [y, y + b, y + h - b, y + h];
        let us = [r[0], r[0] + border, r[0] + r[2] - border, r[0] + r[2]];
        let vs = [r[1], r[1] + border, r[1] + r[3] - border, r[1] + r[3]];
        for j in 0..3 {
            for i in 0..3 {
                self.sprite(xs[i], ys[j], xs[i + 1] - xs[i], ys[j + 1] - ys[j], [us[i], vs[j], us[i + 1] - us[i], vs[j + 1] - vs[j]], color);
            }
        }
    }

    /// Block texture layer drawn as a flat square.
    pub fn tile(&mut self, x: f32, y: f32, size: f32, layer: u16, color: Rgba) {
        self.quad([[x, y + size, 0.0], [x + size, y + size, 0.0], [x + size, y, 0.0], [x, y, 0.0]], [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]], layer as f32, color);
    }

    /// Text at `scale` texels per font texel; returns the x position after the last glyph.
    pub fn text(&mut self, font: &Font, mut x: f32, y: f32, text: &str, scale: f32, color: Rgba) -> f32 {
        let s = 1.0 / ATLAS_SIZE as f32;
        for ch in text.chars() {
            let (index, g) = font.glyph(ch);
            if ch != ' ' {
                let w = (g.width - 1) as f32;
                let u0 = ((index % 16) * GLYPH_CELL) as f32 + g.start as f32;
                let v0 = ((index / 16) * GLYPH_CELL) as f32;
                let (x1, y1) = (x + w * scale, y + 8.0 * scale);
                self.quad([[x, y1, 0.0], [x1, y1, 0.0], [x1, y, 0.0], [x, y, 0.0]], [[u0 * s, (v0 + 8.0) * s], [(u0 + w) * s, (v0 + 8.0) * s], [(u0 + w) * s, v0 * s], [u0 * s, v0 * s]], LAYER_ATLAS, color);
            }
            x += g.width as f32 * scale;
        }
        x
    }

    /// Text with a drop shadow, the default look for HUD text.
    pub fn text_shadow(&mut self, font: &Font, x: f32, y: f32, text: &str, scale: f32, color: Rgba) -> f32 {
        let shadow = [color[0] / 4, color[1] / 4, color[2] / 4, color[3]];
        self.text(font, x + scale, y + scale, text, scale, shadow);
        self.text(font, x, y, text, scale, color)
    }
}

impl Default for Batch {
    fn default() -> Self {
        Batch::new()
    }
}
