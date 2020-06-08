use crate::render::Render;
use anyhow::{anyhow, Result};
use ttf_parser::Font as TtfFont;

/// A parsed TTF font file containing information for displaying text.
pub struct Font {
    /// Amount of glyphs in the font.
    glyphs: u16,
}

impl Font {
    /// Parse a TTF string.
    pub fn from_bytes(font: &[u8]) -> Result<Self> {
        let font = TtfFont::from_data(&font, 0).ok_or(anyhow!("Could not parse font"))?;

        let glyphs = font.number_of_glyphs();

        Ok(Self { glyphs })
    }

    /// Upload it and get a mesh.
    pub fn upload(self, render: &mut Render) -> Result<FontInstance> {
        todo!()
    }
}

/// Font with references to glyph meshes on the GPU.
pub struct FontInstance {}
