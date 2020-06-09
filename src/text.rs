use crate::render::{Mesh, Render};
use anyhow::{anyhow, Result};
use lyon::{math::Point, path::PathEvent};
use std::{collections::HashMap, str::Chars};
use ttf_parser::{Font as TtfFont, GlyphId, OutlineBuilder};
use usvg::Color;

/// A parsed TTF font file containing information for displaying text.
pub struct Font<'a> {
    /// Internal parsed font representation.
    font: TtfFont<'a>,
}

impl<'a> Font<'a> {
    /// Parse a TTF string.
    pub fn from_bytes(font: &'a [u8]) -> Result<Self> {
        let font = TtfFont::from_data(&font, 0).ok_or(anyhow!("Could not parse font"))?;

        Ok(Self { font })
    }

    /// Upload it and get a mesh.
    pub fn upload(mut self, render: &mut Render, chars: Chars) -> Result<FontInstance> {
        let mut meshes = HashMap::new();

        // Upload the requested glyphs
        for ch in chars {
            meshes.insert(
                ch,
                self.upload_glyph(
                    render,
                    self.font
                        .glyph_index(ch)
                        .ok_or(anyhow!("Glyph not found"))?,
                )?,
            );
        }

        Ok(FontInstance { meshes })
    }

    /// Upload a specific glyph.
    fn upload_glyph(&mut self, render: &mut Render, glyph: GlyphId) -> Result<Mesh> {
        let mut builder = GlyphBuilder::new();

        // Convert the glyph to a lyon path
        self.font
            .outline_glyph(glyph, &mut builder)
            .ok_or(anyhow!("Could not build outline of glyph"))?;

        Ok(render.upload_path(builder.path().into_iter(), Color::white(), 1.0))
    }
}

/// Font with references to glyph meshes on the GPU.
pub struct FontInstance {
    /// List of meshes matching the characters.
    meshes: HashMap<char, Mesh>,
}

impl FontInstance {
    /// Get the mesh belonging to a letter.
    pub fn letter_mesh(&self, letter: char) -> Option<Mesh> {
        self.meshes.get(&letter).map(|mesh| *mesh)
    }
}

/// Builder struct for creating lyon paths from a font glyph.
struct GlyphBuilder {
    path: Vec<PathEvent>,
    prev: Point,
    first: Point,
    needs_end: bool,
    deferred: Option<PathEvent>,
}

impl GlyphBuilder {
    /// Setup a new builder.
    pub fn new() -> Self {
        Self {
            path: Vec::new(),
            prev: Point::default(),
            first: Point::default(),
            needs_end: false,
            deferred: None,
        }
    }

    /// Get the built path.
    pub fn path(self) -> Vec<PathEvent> {
        assert!(!self.needs_end);

        self.path
    }
}

impl OutlineBuilder for GlyphBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.needs_end {
            let last = self.prev;
            let first = self.first;
            self.needs_end = false;
            self.prev = Point::new(x, -y);
            self.deferred = Some(PathEvent::Begin { at: self.prev });
            self.first = self.prev;
            self.path.push(PathEvent::End {
                last,
                first,
                close: false,
            });
        } else {
            self.first = Point::new(x, -y);
            self.path.push(PathEvent::Begin { at: self.first });
        }
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.needs_end = true;
        let from = self.prev;
        self.prev = Point::new(x, -y);
        self.path.push(PathEvent::Line {
            from,
            to: self.prev,
        });
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.needs_end = true;
        let from = self.prev;
        self.prev = Point::new(x, -y);
        self.path.push(PathEvent::Quadratic {
            from,
            ctrl: Point::new(x1, -y1),
            to: self.prev,
        });
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.needs_end = true;
        let from = self.prev;
        self.prev = Point::new(x, -y);
        self.path.push(PathEvent::Cubic {
            from,
            ctrl1: Point::new(x1, -y1),
            ctrl2: Point::new(x2, -y2),
            to: self.prev,
        });
    }

    fn close(&mut self) {
        self.needs_end = false;
        self.prev = self.first;
        self.path.push(PathEvent::End {
            last: self.prev,
            first: self.first,
            close: true,
        });
    }
}
