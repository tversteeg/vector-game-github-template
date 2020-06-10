use crate::render::{Instance, Mesh, Render};
use anyhow::{anyhow, Result};
use lyon::{math::Point, path::PathEvent};
use std::{collections::HashMap, str::Chars};
use ttf_parser::{Font as TtfFont, GlyphId, OutlineBuilder};
use usvg::Color;

const HEIGHT: f32 = 100.0;

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

        // Scale the height to 100 high
        let height = self.font.x_height().unwrap_or(self.font.height());
        let scale = HEIGHT / height as f32;

        // Upload the requested glyphs
        for ch in chars {
            let glyph_id = self
                .font
                .glyph_index(ch)
                .ok_or(anyhow!("Glyph not found"))?;
            let mesh = self.upload_glyph(render, glyph_id, scale)?;

            let advance =
                self.font
                    .glyph_hor_advance(glyph_id)
                    .ok_or(anyhow!("Font is missing horizontal advance"))? as f32
                    * scale;
            let side_bearing =
                self.font.glyph_hor_side_bearing(glyph_id).unwrap_or(0) as f32 * scale;

            meshes.insert(
                ch,
                Glyph {
                    mesh,
                    advance,
                    side_bearing,
                },
            );
        }

        Ok(FontInstance {
            meshes,
            space_width: 1.0 * HEIGHT,
        })
    }

    /// Upload a specific glyph.
    fn upload_glyph(&mut self, render: &mut Render, glyph: GlyphId, scale: f32) -> Result<Mesh> {
        let mut builder = GlyphBuilder::new(scale);

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
    meshes: HashMap<char, Glyph>,
    /// The horizontal font line gap.
    space_width: f32,
}

impl FontInstance {
    /// Get the mesh belonging to a letter.
    pub fn letter_mesh(&self, letter: char) -> Option<Mesh> {
        self.meshes.get(&letter).map(|glyph| glyph.mesh)
    }

    /// Form the mesh letters into the text.
    pub fn text(&self, text: &str, x: f32, y: f32) -> Vec<(Instance, Mesh)> {
        let mut result = Vec::new();

        let mut letter_x = x;

        for ch in text.chars() {
            // Find the character
            if let Some(glyph) = self.meshes.get(&ch) {
                result.push((Instance::new(letter_x + glyph.side_bearing, y), glyph.mesh));

                letter_x += glyph.advance;
            } else {
                // Used for not defined letters and whitespace
                letter_x += self.space_width;
            }
        }

        result
    }
}

/// A glyph for a character.
#[derive(Debug)]
struct Glyph {
    /// The reference to the GPU mesh.
    mesh: Mesh,
    /// The advance of the font.
    advance: f32,
    /// Horizontal side bearing.
    side_bearing: f32,
}

/// Builder struct for creating lyon paths from a font glyph.
struct GlyphBuilder {
    path: Vec<PathEvent>,
    prev: Point,
    first: Point,
    scale: f32,
    needs_end: bool,
    deferred: Option<PathEvent>,
}

impl GlyphBuilder {
    /// Setup a new builder.
    pub fn new(scale: f32) -> Self {
        Self {
            scale,
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
            self.prev = Point::new(x * self.scale, -y * self.scale);
            self.deferred = Some(PathEvent::Begin { at: self.prev });
            self.first = self.prev;
            self.path.push(PathEvent::End {
                last,
                first,
                close: false,
            });
        } else {
            self.first = Point::new(x * self.scale, -y * self.scale);
            self.path.push(PathEvent::Begin { at: self.first });
        }
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.needs_end = true;
        let from = self.prev;
        self.prev = Point::new(x * self.scale, -y * self.scale);
        self.path.push(PathEvent::Line {
            from,
            to: self.prev,
        });
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.needs_end = true;
        let from = self.prev;
        self.prev = Point::new(x * self.scale, -y * self.scale);
        self.path.push(PathEvent::Quadratic {
            from,
            ctrl: Point::new(x1 * self.scale, -y1 * self.scale),
            to: self.prev,
        });
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.needs_end = true;
        let from = self.prev;
        self.prev = Point::new(x * self.scale, -y * self.scale);
        self.path.push(PathEvent::Cubic {
            from,
            ctrl1: Point::new(x1 * self.scale, -y1 * self.scale),
            ctrl2: Point::new(x2 * self.scale, -y2 * self.scale),
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
