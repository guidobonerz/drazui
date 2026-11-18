//! Hardware-accelerated renderer.
//!
//! [`Mesh`] turns a frame into textured triangles plus a glyph [`Atlas`]; that
//! part is API-independent and can feed any GPU API. [`Renderer`] draws it
//! with OpenGL through [miniquad](https://docs.rs/miniquad): one vertex
//! buffer, one texture and one draw call per clip rectangle.

use std::collections::HashMap;

use miniquad::*;

#[cfg(feature = "ttf")]
pub use crate::font::Font;
use crate::{Color, Draw, Rect, Ui};

/// Side length of the square glyph atlas in pixels.
pub const ATLAS_SIZE: usize = 1024;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    /// Position in device pixels, origin top-left.
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    /// Straight (not premultiplied) RGBA, 0..1.
    pub color: [f32; 4],
}

/// A run of indices drawn with one scissor rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Batch {
    /// Scissor rectangle in device pixels, origin top-left.
    pub clip: Rect,
    pub start: usize,
    pub count: usize,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(feature = "ttf"), allow(dead_code))]
struct Slot {
    /// u0, v0, u1, v1
    uv: [f32; 4],
    w: f32,
    h: f32,
    xmin: f32,
    ymin: f32,
    advance: f32,
}

/// Glyph texture: RGBA, white with the glyph coverage in alpha. The texel at
/// (0, 0) is opaque white and used for solid rectangles.
pub struct Atlas {
    pub pixels: Vec<u8>,
    /// Set when `pixels` changed and must be uploaded again.
    pub dirty: bool,
    slots: HashMap<(char, u32), Slot>,
    /// Shelf packer cursor.
    x: usize,
    y: usize,
    shelf_h: usize,
    full: bool,
    #[cfg_attr(not(feature = "ttf"), allow(dead_code))]
    font_key: usize,
}

impl Atlas {
    fn new() -> Atlas {
        let mut a = Atlas {
            pixels: vec![0; ATLAS_SIZE * ATLAS_SIZE * 4],
            dirty: true,
            slots: HashMap::new(),
            x: 0,
            y: 0,
            shelf_h: 0,
            full: false,
            font_key: 0,
        };
        a.clear();
        a
    }

    fn clear(&mut self) {
        self.pixels.fill(0);
        self.pixels[..4].fill(255);
        self.slots.clear();
        (self.x, self.y, self.shelf_h) = (2, 0, 2);
        self.full = false;
        self.dirty = true;
    }

    /// Copies a `w` x `h` coverage bitmap into the atlas and returns its UVs.
    fn insert(&mut self, w: usize, h: usize, cov: impl Fn(usize, usize) -> u8) -> Option<[f32; 4]> {
        if self.x + w + 1 > ATLAS_SIZE {
            self.x = 0;
            self.y += self.shelf_h + 1;
            self.shelf_h = 0;
        }
        if self.y + h + 1 > ATLAS_SIZE || w + 1 > ATLAS_SIZE {
            self.full = true;
            return None;
        }
        for gy in 0..h {
            for gx in 0..w {
                let i = ((self.y + gy) * ATLAS_SIZE + self.x + gx) * 4;
                self.pixels[i..i + 4].copy_from_slice(&[255, 255, 255, cov(gx, gy)]);
            }
        }
        let s = ATLAS_SIZE as f32;
        let uv = [self.x as f32 / s, self.y as f32 / s, (self.x + w) as f32 / s, (self.y + h) as f32 / s];
        self.x += w + 1;
        self.shelf_h = self.shelf_h.max(h);
        self.dirty = true;
        Some(uv)
    }

    /// Glyph of the built-in 8x8 font (stored once, scaled by the quad).
    fn bitmap(&mut self, ch: char) -> Option<Slot> {
        if let Some(s) = self.slots.get(&(ch, 0)) {
            return Some(*s);
        }
        let g = crate::soft::glyph(ch);
        let uv = self.insert(8, 8, |x, y| if g[y] >> x & 1 != 0 { 255 } else { 0 })?;
        let slot = Slot { uv, w: 8.0, h: 8.0, xmin: 0.0, ymin: 0.0, advance: 8.0 };
        self.slots.insert((ch, 0), slot);
        Some(slot)
    }

    /// Glyph of a vector font, rasterized at `size` pixels.
    #[cfg(feature = "ttf")]
    fn vector(&mut self, font: &Font, ch: char, size: f32) -> Option<Slot> {
        let px = size.round().max(1.0) as u32;
        if let Some(s) = self.slots.get(&(ch, px)) {
            return Some(*s);
        }
        let g = font.glyph(ch, size);
        let uv = if g.w * g.h == 0 { [0.0; 4] } else { self.insert(g.w, g.h, |x, y| g.cov[y * g.w + x])? };
        let slot = Slot { uv, w: g.w as f32, h: g.h as f32, xmin: g.xmin as f32, ymin: g.ymin as f32, advance: g.advance };
        self.slots.insert((ch, px), slot);
        Some(slot)
    }
}

/// Triangles for one frame.
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub batches: Vec<Batch>,
    pub atlas: Atlas,
}

impl Default for Mesh {
    fn default() -> Self {
        Mesh::new()
    }
}

impl Mesh {
    pub fn new() -> Mesh {
        Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            batches: Vec::new(),
            atlas: Atlas::new(),
        }
    }

    /// Rebuilds the mesh from the last finished frame of `ui` for a surface
    /// of `width` x `height` device pixels.
    pub fn build(&mut self, ui: &Ui, width: f32, height: f32) {
        #[cfg(feature = "ttf")]
        {
            let key = ui.font().map_or(0, Font::key);
            if key != self.atlas.font_key {
                self.atlas.clear();
                self.atlas.font_key = key;
            }
        }
        self.tessellate(ui, width, height);
        if self.atlas.full {
            // out of space: start over with only the glyphs of this frame
            self.atlas.clear();
            self.tessellate(ui, width, height);
        }
    }

    fn quad(&mut self, r: [f32; 4], uv: [f32; 4], color: [f32; 4]) {
        let base = self.vertices.len() as u32;
        let v = |x: usize, y: usize| Vertex { pos: [r[x], r[y]], uv: [uv[x], uv[y]], color };
        self.vertices.extend([v(0, 1), v(2, 1), v(2, 3), v(0, 3)]);
        self.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn end_batch(&mut self, clip: Rect, start: usize) {
        let count = self.indices.len() - start;
        if count > 0 && clip.w > 0.0 && clip.h > 0.0 {
            self.batches.push(Batch { clip, start, count });
        }
    }

    fn tessellate(&mut self, ui: &Ui, width: f32, height: f32) {
        const WHITE: [f32; 4] = [0.5 / ATLAS_SIZE as f32; 4];
        self.vertices.clear();
        self.indices.clear();
        self.batches.clear();
        let screen = Rect::new(0.0, 0.0, width, height);
        let mut clip = screen;
        let mut start = 0;
        #[cfg(feature = "ttf")]
        let font = ui.font();
        for d in ui.draws() {
            match d {
                Draw::Clip(r) => {
                    self.end_batch(clip, start);
                    start = self.indices.len();
                    clip = r.intersect(screen);
                }
                Draw::Rect { rect, color } => {
                    if rect.overlaps(clip) {
                        self.quad([rect.x, rect.y, rect.right(), rect.bottom()], WHITE, rgba(color));
                    }
                }
                Draw::Text { pos, text, size, color } => {
                    #[cfg(feature = "ttf")]
                    if let Some(font) = font {
                        self.vector_text(font, pos.x, pos.y, text, size, rgba(color), clip);
                        continue;
                    }
                    self.text(pos.x, pos.y, text, size, rgba(color), clip)
                }
            }
        }
        self.end_batch(clip, start);
    }

    #[cfg(feature = "ttf")]
    #[allow(clippy::too_many_arguments)]
    fn vector_text(&mut self, font: &Font, x: f32, y: f32, text: &str, size: f32, color: [f32; 4], clip: Rect) {
        let base = y + font.baseline(size) as f32;
        let mut pen = x;
        for ch in text.chars() {
            if pen >= clip.right() {
                break;
            }
            let Some(s) = self.atlas.vector(font, ch, size) else { continue };
            let gx = pen.round() + s.xmin;
            let gy = base - s.ymin - s.h;
            if s.w > 0.0 && gx + s.w > clip.x {
                self.quad([gx, gy, gx + s.w, gy + s.h], s.uv, color);
            }
            pen += s.advance;
        }
    }

    /// Text in the 8x8 bitmap font.
    fn text(&mut self, x: f32, y: f32, text: &str, size: f32, color: [f32; 4], clip: Rect) {
        let cell = size.round().max(1.0);
        if y >= clip.bottom() || y + cell <= clip.y {
            return;
        }
        let mut gx = x;
        for ch in text.chars() {
            if gx >= clip.right() {
                break;
            }
            if gx + cell > clip.x && ch != ' ' {
                if let Some(s) = self.atlas.bitmap(ch) {
                    self.quad([gx, y, gx + cell, y + cell], s.uv, color);
                }
            }
            gx += cell;
        }
    }
}

fn rgba(c: Color) -> [f32; 4] {
    [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, c.a() as f32 / 255.0]
}

const VERTEX: &str = "#version 100
attribute vec2 in_pos;
attribute vec2 in_uv;
attribute vec4 in_color;
uniform vec2 screen;
varying mediump vec2 uv;
varying lowp vec4 color;
void main() {
    vec2 p = in_pos / screen * 2.0 - 1.0;
    gl_Position = vec4(p.x, -p.y, 0.0, 1.0);
    uv = in_uv;
    color = in_color;
}";

const FRAGMENT: &str = "#version 100
varying mediump vec2 uv;
varying lowp vec4 color;
uniform sampler2D tex;
void main() {
    gl_FragColor = color * texture2D(tex, uv);
}";

/// Draws frames with OpenGL through miniquad.
///
/// ```ignore
/// // setup
/// let mut ctx = miniquad::window::new_rendering_backend();
/// let mut renderer = drazui::gpu::Renderer::new(&mut *ctx);
/// // every frame, after ui.end()
/// let (w, h) = miniquad::window::screen_size();
/// ctx.begin_default_pass(Default::default());
/// renderer.render(&mut *ctx, &ui, w, h);
/// ctx.end_render_pass();
/// ctx.commit_frame();
/// ```
pub struct Renderer {
    pub mesh: Mesh,
    pipeline: Pipeline,
    bindings: Bindings,
    vertex_cap: usize,
    index_cap: usize,
}

impl Renderer {
    pub fn new(ctx: &mut dyn RenderingBackend) -> Renderer {
        let mesh = Mesh::new();
        let texture = ctx.new_texture_from_rgba8(ATLAS_SIZE as u16, ATLAS_SIZE as u16, &mesh.atlas.pixels);
        ctx.texture_set_filter(texture, FilterMode::Nearest, MipmapFilterMode::None);
        let (vertex_cap, index_cap) = (4096, 6144);
        let bindings = Bindings {
            vertex_buffers: vec![ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Stream, BufferSource::empty::<Vertex>(vertex_cap))],
            index_buffer: ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Stream, BufferSource::empty::<u32>(index_cap)),
            images: vec![texture],
        };
        let meta = ShaderMeta {
            images: vec!["tex".to_string()],
            uniforms: UniformBlockLayout { uniforms: vec![UniformDesc::new("screen", UniformType::Float2)] },
        };
        let shader = ctx.new_shader(ShaderSource::Glsl { vertex: VERTEX, fragment: FRAGMENT }, meta).expect("drazui shader");
        let pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("in_pos", VertexFormat::Float2),
                VertexAttribute::new("in_uv", VertexFormat::Float2),
                VertexAttribute::new("in_color", VertexFormat::Float4),
            ],
            shader,
            PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
        );
        Renderer { mesh, pipeline, bindings, vertex_cap, index_cap }
    }

    /// Draws the last finished frame of `ui`. Call inside a render pass whose
    /// target is `width` x `height` pixels (the same size given to `ui.begin`).
    pub fn render(&mut self, ctx: &mut dyn RenderingBackend, ui: &Ui, width: f32, height: f32) {
        self.mesh.build(ui, width, height);
        let mesh = &mut self.mesh;
        if mesh.atlas.dirty {
            ctx.texture_update(self.bindings.images[0], &mesh.atlas.pixels);
            mesh.atlas.dirty = false;
        }
        // grow the buffers when the frame does not fit
        if mesh.vertices.len() > self.vertex_cap {
            self.vertex_cap = mesh.vertices.len().next_power_of_two();
            ctx.delete_buffer(self.bindings.vertex_buffers[0]);
            self.bindings.vertex_buffers[0] =
                ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Stream, BufferSource::empty::<Vertex>(self.vertex_cap));
        }
        if mesh.indices.len() > self.index_cap {
            self.index_cap = mesh.indices.len().next_power_of_two();
            ctx.delete_buffer(self.bindings.index_buffer);
            self.bindings.index_buffer =
                ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Stream, BufferSource::empty::<u32>(self.index_cap));
        }
        if mesh.batches.is_empty() {
            return;
        }
        ctx.buffer_update(self.bindings.vertex_buffers[0], BufferSource::slice(&mesh.vertices));
        ctx.buffer_update(self.bindings.index_buffer, BufferSource::slice(&mesh.indices));
        ctx.apply_pipeline(&self.pipeline);
        ctx.apply_bindings(&self.bindings);
        ctx.apply_uniforms(UniformsSource::table(&[width, height]));
        for b in &mesh.batches {
            // GL scissor rectangles have their origin at the bottom-left
            let c = b.clip;
            ctx.apply_scissor_rect(c.x as i32, (height - c.bottom()) as i32, c.w as i32, c.h as i32);
            ctx.draw(b.start as i32, b.count as i32, 1);
        }
        ctx.apply_scissor_rect(0, 0, width as i32, height as i32);
    }
}
