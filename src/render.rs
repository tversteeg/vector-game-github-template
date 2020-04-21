use lyon::{
    math::Point,
    path::PathEvent,
    tessellation::{BuffersBuilder, FillAttributes, FillOptions, FillTessellator, VertexBuffers},
};
use miniquad::{graphics::*, Context};

const VERTEX: &str = r#"#version 100
attribute vec2 pos;

uniform vec2 resolution;

void main() {
    gl_Position = vec4(pos + resolution, 0, 1);
}
"#;

const FRAGMENT: &str = r#"#version 100

void main() {
    gl_FragColor = vec4(1.0, 0.0, 0.0, 0.0);
}"#;

const META: ShaderMeta = ShaderMeta {
    images: &[],
    uniforms: UniformBlockLayout {
        uniforms: &[("resolution", UniformType::Float2)],
    },
};

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct Vertex {
    pos: [f32; 2],
    normal: [f32; 2],
    prim_id: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct Primitive {
    color: [f32; 4],
    translate: [f32; 2],
    z_index: i32,
    width: f32,
}

#[repr(C)]
struct Uniforms {
    resolution: (f32, f32),
}

struct DrawCall {
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
    bindings: Option<Bindings>,
}

impl DrawCall {
    /// Create bindings if they are missing.
    pub fn create_bindings(&mut self, ctx: &mut Context) {
        let vertex_buffer = Buffer::stream(
            ctx,
            BufferType::VertexBuffer,
            self.vertices.len() * std::mem::size_of::<Vertex>(),
        );
        let index_buffer = Buffer::stream(
            ctx,
            BufferType::IndexBuffer,
            self.indices.len() * std::mem::size_of::<u16>(),
        );
        let bindings = Bindings {
            vertex_buffers: vec![vertex_buffer],
            index_buffer,
            images: vec![],
        };
        self.bindings = Some(bindings);
    }
}

/// A wrapper around the OpenGL calls so the main file won't be polluted.
pub struct Render {
    pipeline: Pipeline,
    draw_calls: Vec<DrawCall>,
}

impl Render {
    /// Setup the OpenGL pipeline and the texture for the framebuffer.
    pub fn new(ctx: &mut Context) -> Self {
        // Create an OpenGL pipeline
        let shader = Shader::new(ctx, VERTEX, FRAGMENT, META);
        let pipeline = Pipeline::new(
            ctx,
            &[BufferLayout::default()],
            &[VertexAttribute::new("pos", VertexFormat::Float2)],
            shader,
        );

        Self {
            pipeline,
            draw_calls: vec![],
        }
    }

    pub fn upload<P>(&mut self, path: P)
    where
        P: IntoIterator<Item = PathEvent>,
    {
        // Tessalate the path, converting it to vertices & indices
        let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();
        let mut tessellator = FillTessellator::new();
        {
            tessellator
                .tessellate(
                    path,
                    &FillOptions::default(),
                    &mut BuffersBuilder::new(&mut geometry, |pos: Point, _: FillAttributes| {
                        Vertex {
                            pos: pos.to_array(),
                            ..Default::default()
                        }
                    }),
                )
                .unwrap();
        }
        let vertices = geometry.vertices.clone();
        let indices = geometry.indices.clone();

        // Create an OpenGL draw call for the path
        self.draw_calls.push(DrawCall {
            vertices,
            indices,
            bindings: None,
        });
    }

    /// Render the graphics.
    pub fn render(&mut self, ctx: &mut Context) {
        // Render the texture quad
        ctx.begin_default_pass(PassAction::Nothing);

        let (width, height) = ctx.screen_size();

        // Create the bindings if they don't exist
        self.draw_calls
            .iter_mut()
            .filter(|dc| dc.bindings.is_none())
            .for_each(|dc| dc.create_bindings(ctx));

        for dc in self.draw_calls.iter_mut() {
            ctx.apply_pipeline(&self.pipeline);
            ctx.apply_bindings(dc.bindings.as_ref().unwrap());
            ctx.apply_uniforms(&Uniforms {
                resolution: (width, height),
            });
            ctx.draw(0, dc.indices.len() as i32, 1);
        }

        ctx.end_render_pass();

        ctx.commit_frame();
    }
}
