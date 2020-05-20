use anyhow::Result;
use legion::{
    filter::filter_fns::tag_value,
    query::{IntoQuery, Read},
    world::World,
};
use lyon::{
    math::Point,
    path::PathEvent,
    tessellation::{
        geometry_builder::{FillVertexConstructor, StrokeVertexConstructor},
        BuffersBuilder, FillAttributes, FillOptions, FillTessellator, LineCap, LineJoin,
        StrokeAttributes, StrokeOptions, StrokeTessellator, VertexBuffers,
    },
};
use miniquad::{graphics::*, Context};
use std::mem;
use usvg::{Color, NodeKind, Options, Paint, ShapeRendering, Tree};

const PATH_TOLERANCE: f32 = 0.01;
const MAX_MESH_INSTANCES: usize = 1024 * 1024;

/// A reference to an uploaded vector path.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Mesh(usize);

/// A wrapper around the OpenGL calls so the main file won't be polluted.
pub struct Render {
    /// The OpenGL pipeline for the pass rendering to the render target.
    offscreen_pipeline: Pipeline,
    /// The OpenGL pass for rendering to the render target.
    offscreen_pass: RenderPass,
    /// The OpenGL pipeline to apply FXAA as a post-processing effect.
    post_processing_pipeline: Pipeline,
    /// The bindings to the GPU containing the render target.
    post_processing_bind: Bindings,
    /// A list of draw calls with bindings that will be generated.
    draw_calls: Vec<DrawCall>,
    /// Whether some draw calls are missing bindings.
    missing_bindings: bool,
}

impl Render {
    /// Setup the OpenGL pipeline and the texture for the framebuffer.
    pub fn new(ctx: &mut Context) -> Self {
        // Create a first pass to render to a render target
        let (width, height) = ctx.screen_size();
        let (color_img, offscreen_pass) = Self::create_offscreen_pass(ctx, width as _, height as _);

        // Create an OpenGL pipeline for rendering to the render target
        let offscreen_shader = Shader::new(
            ctx,
            geom_shader::VERTEX,
            geom_shader::FRAGMENT,
            geom_shader::META,
        );
        let offscreen_pipeline = Pipeline::with_params(
            ctx,
            &[
                BufferLayout::default(),
                BufferLayout {
                    step_func: VertexStep::PerInstance,
                    ..Default::default()
                },
            ],
            &[
                VertexAttribute::with_buffer("a_pos", VertexFormat::Float2, 0),
                VertexAttribute::with_buffer("a_color", VertexFormat::Float4, 0),
                VertexAttribute::with_buffer("a_inst_pos", VertexFormat::Float2, 1),
                VertexAttribute::with_buffer("a_inst_rot", VertexFormat::Float1, 1),
                VertexAttribute::with_buffer("a_inst_scale", VertexFormat::Float1, 1),
            ],
            offscreen_shader,
            PipelineParams {
                depth_test: Comparison::LessOrEqual,
                depth_write: true,
                ..Default::default()
            },
        );

        // Create an OpenGL pipeline for post-processing effects on the render target
        let post_processing_shader = Shader::new(
            ctx,
            post_process_shader::VERTEX,
            post_process_shader::FRAGMENT,
            post_process_shader::META,
        );

        #[rustfmt::skip]
        let vertices: &[f32] = &[
            /* pos         uvs */
            -1.0, -1.0,    0.0, 0.0,
             1.0, -1.0,    1.0, 0.0,
             1.0,  1.0,    1.0, 1.0,
            -1.0,  1.0,    0.0, 1.0,
        ];
        let vertex_buffer = Buffer::immutable(ctx, BufferType::VertexBuffer, &vertices);

        let indices: &[u16] = &[0, 1, 2, 0, 2, 3];
        let index_buffer = Buffer::immutable(ctx, BufferType::IndexBuffer, &indices);

        let post_processing_bind = Bindings {
            vertex_buffers: vec![vertex_buffer],
            index_buffer,
            images: vec![color_img],
        };

        let post_processing_pipeline = Pipeline::new(
            ctx,
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("a_pos", VertexFormat::Float2),
                VertexAttribute::new("a_uv", VertexFormat::Float2),
            ],
            post_processing_shader,
        );

        Self {
            offscreen_pass,
            offscreen_pipeline,
            post_processing_pipeline,
            post_processing_bind,
            draw_calls: vec![],
            missing_bindings: false,
        }
    }

    /// Upload a lyon path.
    ///
    /// Returns a reference that can be used to add instances.
    pub fn upload_path<P>(&mut self, path: P, color: Color, opacity: f32) -> Mesh
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
                    &mut BuffersBuilder::new(&mut geometry, VertexCtor::new(color, opacity)),
                )
                .unwrap();
        }
        let vertices = geometry.vertices.clone();
        let indices = geometry.indices.clone();

        // Create an OpenGL draw call for the path
        let draw_call = DrawCall {
            vertices,
            indices,
            bindings: None,
            instances: vec![],
            refresh_instances: false,
        };
        self.draw_calls.push(draw_call);

        // Tell the next render loop to create bindings for this
        self.missing_bindings = true;

        // Return the draw call in a newtype struct so it can be used as a reference
        Mesh(self.draw_calls.len() - 1)
    }

    /// Upload a SVG.
    ///
    /// Returns a reference that can be used to add instances.
    pub fn upload_svg<S>(&mut self, svg: S) -> Result<Mesh>
    where
        S: AsRef<str>,
    {
        // Tessalate the path, converting it to vertices & indices
        let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();

        let mut fill_tess = FillTessellator::new();
        let mut stroke_tess = StrokeTessellator::new();

        // Parse the SVG string
        let options = Options {
            shape_rendering: ShapeRendering::GeometricPrecision,
            keep_named_groups: true,
            ..Default::default()
        };
        let rtree = Tree::from_str(svg.as_ref(), &options)?;
        // Loop over all nodes in the SVG tree
        for node in rtree.root().descendants() {
            if let NodeKind::Path(ref path) = *node.borrow() {
                if let Some(ref fill) = path.fill {
                    // Get the fill color
                    let color = match fill.paint {
                        Paint::Color(color) => color,
                        _ => todo!("Color not defined"),
                    };

                    // Tessellate the fill
                    fill_tess
                        .tessellate(
                            convert_path(path),
                            &FillOptions::tolerance(PATH_TOLERANCE),
                            &mut BuffersBuilder::new(
                                &mut geometry,
                                VertexCtor::new(color, fill.opacity.value() as f32),
                            ),
                        )
                        .expect("Tessellation failed");
                }

                if let Some(ref stroke) = path.stroke {
                    let (color, stroke_opts) = convert_stroke(stroke);
                    // Tessellate the stroke
                    let _ = stroke_tess.tessellate(
                        convert_path(path),
                        &stroke_opts.with_tolerance(PATH_TOLERANCE),
                        &mut BuffersBuilder::new(
                            &mut geometry,
                            VertexCtor::new(color, stroke.opacity.value() as f32),
                        ),
                    );
                }
            }
        }

        let vertices = geometry.vertices.clone();
        let indices = geometry.indices.clone();

        // Create an OpenGL draw call for the path
        let draw_call = DrawCall {
            vertices,
            indices,
            bindings: None,
            instances: vec![],
            refresh_instances: false,
        };
        self.draw_calls.push(draw_call);

        // Tell the next render loop to create bindings for this
        self.missing_bindings = true;

        // Return the draw call in a newtype struct so it can be used as a reference
        Ok(Mesh(self.draw_calls.len() - 1))
    }

    /// Update the instances for each draw call.
    pub fn update(&mut self, world: &mut World) {
        // Get all instances and meshes
        self.draw_calls
            .iter_mut()
            .enumerate()
            .for_each(|(index, mut draw_call)| {
                let mesh = Mesh(index);

                // Get the meshes belongin to the draw call
                let query = <Read<Instance>>::query().filter(tag_value(&mesh));

                // Copy the instances from legion to the draw call
                // TODO add a better mechanism to detect manual changes
                if !draw_call.refresh_instances {
                    draw_call.instances = query.iter(world).map(|pos| *pos).collect();
                }

                // Tell the render loop that the position of the instances have been changed
                draw_call.refresh_instances = true;
            });
    }

    /// Render the graphics.
    pub fn render(&mut self, ctx: &mut Context) {
        let (width, height) = ctx.screen_size();

        // Create bindings & update the instance vertices if necessary
        if self.missing_bindings {
            self.draw_calls.iter_mut().for_each(|dc| {
                // Create bindings if missing
                if dc.bindings.is_none() {
                    dc.create_bindings(ctx);
                }
            });

            self.missing_bindings = false;
        }

        // Render the pass to the render target
        ctx.begin_pass(
            self.offscreen_pass,
            PassAction::clear_color(0.4, 0.7, 1.0, 1.0),
        );

        // Render the separate draw calls
        for dc in self.draw_calls.iter_mut() {
            // Only render when we actually have instances
            if dc.instances.is_empty() {
                dbg!(dc);
                continue;
            }

            let bindings = dc.bindings.as_ref().unwrap();
            if dc.refresh_instances {
                // Upload the instance positions
                bindings.vertex_buffers[1].update(ctx, &dc.instances);

                dc.refresh_instances = false;
            }

            ctx.apply_pipeline(&self.offscreen_pipeline);
            ctx.apply_scissor_rect(0, 0, width as i32, height as i32);
            ctx.apply_bindings(bindings);
            ctx.apply_uniforms(&geom_shader::Uniforms {
                zoom: (2.0 / width, 2.0 / height),
                pan: (-width / 2.0, -height / 2.0),
            });
            ctx.draw(0, dc.indices.len() as i32, dc.instances.len() as i32);
        }

        ctx.end_render_pass();

        // Render the post-processing pass
        ctx.begin_default_pass(PassAction::Nothing);
        ctx.apply_pipeline(&self.post_processing_pipeline);
        ctx.apply_bindings(&self.post_processing_bind);
        ctx.apply_uniforms(&post_process_shader::Uniforms {
            resolution: (width, height),
        });
        ctx.draw(0, 6, 1);
        ctx.end_render_pass();

        ctx.commit_frame();
    }

    /// Handle the resize event, needed for resizing the render target.
    pub fn resize(&mut self, ctx: &mut Context, width: f32, height: f32) {
        let (color_img, offscreen_pass) = Self::create_offscreen_pass(ctx, width as _, height as _);

        self.offscreen_pass.delete(ctx);
        self.offscreen_pass = offscreen_pass;
        self.post_processing_bind.images[0] = color_img;
    }

    /// Overwrite the instances.
    pub fn set_instances(&mut self, mesh: &Mesh, instances: Vec<Instance>) {
        let mut dc = &mut self.draw_calls[mesh.0];

        dc.instances = instances;
        dc.refresh_instances = true;
    }

    /// Create a render pass for the offscreen texture, used when resizing.
    fn create_offscreen_pass(ctx: &mut Context, width: u32, height: u32) -> (Texture, RenderPass) {
        let color_img = Texture::new_render_texture(
            ctx,
            TextureParams {
                width,
                height,
                format: TextureFormat::RGBA8,
                ..Default::default()
            },
        );
        let depth_img = Texture::new_render_texture(
            ctx,
            TextureParams {
                width,
                height,
                format: TextureFormat::Depth,
                ..Default::default()
            },
        );

        (color_img, RenderPass::new(ctx, color_img, depth_img))
    }
}

/// A single uploaded mesh as a draw call.
#[derive(Debug)]
struct DrawCall {
    /// Render vertices, build by lyon path.
    vertices: Vec<Vertex>,
    /// Render indices, build by lyon path.
    indices: Vec<u16>,
    /// Render bindings, generated on render loop if empty.
    bindings: Option<Bindings>,
    /// List of instances to render.
    instances: Vec<Instance>,
    /// Whether the instance information should be reuploaded to the GPU.
    refresh_instances: bool,
}

impl DrawCall {
    /// Create bindings if they are missing.
    fn create_bindings(&mut self, ctx: &mut Context) {
        // The vertex buffer of the vector paths
        let vertex_buffer = Buffer::immutable(ctx, BufferType::VertexBuffer, &self.vertices);
        // The index buffer of the vector paths
        let index_buffer = Buffer::immutable(ctx, BufferType::IndexBuffer, &self.indices);

        // A dynamic buffer that will contain all positions for all instances
        let instance_positions = Buffer::stream(
            ctx,
            BufferType::VertexBuffer,
            MAX_MESH_INSTANCES * mem::size_of::<Instance>(),
        );

        let bindings = Bindings {
            vertex_buffers: vec![vertex_buffer, instance_positions],
            index_buffer,
            images: vec![],
        };
        self.bindings = Some(bindings);
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
struct Vertex {
    pos: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Instance {
    position: [f32; 2],
    rotation: f32,
    scale: f32,
}

impl Instance {
    /// Create a new instance with a position.
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            position: [x, y],
            rotation: 0.0,
            scale: 1.0,
        }
    }

    /// Set the X position.
    pub fn set_x(&mut self, new: f32) {
        self.position[0] = new;
    }

    /// Get the X position.
    pub fn x(&self) -> f32 {
        self.position[0]
    }

    /// Set the Y position.
    pub fn set_y(&mut self, new: f32) {
        self.position[1] = new;
    }

    /// Get the Y position.
    pub fn y(&self) -> f32 {
        self.position[1]
    }

    /// Set the scale.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }

    /// Get the scale.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Set the rotation.
    pub fn set_rotation(&mut self, rotation: f32) {
        self.rotation = rotation;
    }

    /// Get the rotation.
    pub fn rotation(&self) -> f32 {
        self.rotation
    }
}

/// Used by lyon to create vertices.
struct VertexCtor {
    color: [f32; 4],
}

impl VertexCtor {
    pub fn new(color: Color, alpha: f32) -> Self {
        Self {
            color: [
                color.red as f32 / 255.0,
                color.green as f32 / 255.0,
                color.blue as f32 / 255.0,
                alpha,
            ],
        }
    }
}

impl FillVertexConstructor<Vertex> for VertexCtor {
    fn new_vertex(&mut self, position: Point, _: FillAttributes) -> Vertex {
        Vertex {
            pos: position.to_array(),
            color: self.color,
        }
    }
}

impl StrokeVertexConstructor<Vertex> for VertexCtor {
    fn new_vertex(&mut self, position: Point, _: StrokeAttributes) -> Vertex {
        Vertex {
            pos: position.to_array(),
            color: self.color,
        }
    }
}

struct PathConvIter<'a> {
    iter: std::slice::Iter<'a, usvg::PathSegment>,
    prev: Point,
    first: Point,
    needs_end: bool,
    deferred: Option<PathEvent>,
}

impl<'l> Iterator for PathConvIter<'l> {
    type Item = PathEvent;
    fn next(&mut self) -> Option<PathEvent> {
        if self.deferred.is_some() {
            return self.deferred.take();
        }

        let next = self.iter.next();
        match next {
            Some(usvg::PathSegment::MoveTo { x, y }) => {
                if self.needs_end {
                    let last = self.prev;
                    let first = self.first;
                    self.needs_end = false;
                    self.prev = point(x, y);
                    self.deferred = Some(PathEvent::Begin { at: self.prev });
                    self.first = self.prev;
                    Some(PathEvent::End {
                        last,
                        first,
                        close: false,
                    })
                } else {
                    self.first = point(x, y);
                    Some(PathEvent::Begin { at: self.first })
                }
            }
            Some(usvg::PathSegment::LineTo { x, y }) => {
                self.needs_end = true;
                let from = self.prev;
                self.prev = point(x, y);
                Some(PathEvent::Line {
                    from,
                    to: self.prev,
                })
            }
            Some(usvg::PathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            }) => {
                self.needs_end = true;
                let from = self.prev;
                self.prev = point(x, y);
                Some(PathEvent::Cubic {
                    from,
                    ctrl1: point(x1, y1),
                    ctrl2: point(x2, y2),
                    to: self.prev,
                })
            }
            Some(usvg::PathSegment::ClosePath) => {
                self.needs_end = false;
                self.prev = self.first;
                Some(PathEvent::End {
                    last: self.prev,
                    first: self.first,
                    close: true,
                })
            }
            None => {
                if self.needs_end {
                    self.needs_end = false;
                    let last = self.prev;
                    let first = self.first;
                    Some(PathEvent::End {
                        last,
                        first,
                        close: false,
                    })
                } else {
                    None
                }
            }
        }
    }
}

fn point(x: &f64, y: &f64) -> Point {
    Point::new((*x) as f32, (*y) as f32)
}

fn convert_path<'a>(p: &'a usvg::Path) -> PathConvIter<'a> {
    PathConvIter {
        iter: p.data.iter(),
        first: Point::new(0.0, 0.0),
        prev: Point::new(0.0, 0.0),
        deferred: None,
        needs_end: false,
    }
}

fn convert_stroke(s: &usvg::Stroke) -> (Color, StrokeOptions) {
    let color = match s.paint {
        usvg::Paint::Color(c) => c,
        _ => todo!("No fallback color"),
    };
    let linecap = match s.linecap {
        usvg::LineCap::Butt => LineCap::Butt,
        usvg::LineCap::Square => LineCap::Square,
        usvg::LineCap::Round => LineCap::Round,
    };
    let linejoin = match s.linejoin {
        usvg::LineJoin::Miter => LineJoin::Miter,
        usvg::LineJoin::Bevel => LineJoin::Bevel,
        usvg::LineJoin::Round => LineJoin::Round,
    };

    let opt = StrokeOptions::tolerance(PATH_TOLERANCE)
        .with_line_width(s.width.value() as f32)
        .with_line_cap(linecap)
        .with_line_join(linejoin);

    (color, opt)
}

mod geom_shader {
    use miniquad::graphics::*;

    pub const VERTEX: &str = r#"#version 100

uniform vec2 u_zoom;
uniform vec2 u_pan;

attribute vec2 a_pos;
attribute vec4 a_color;
attribute vec2 a_inst_pos;
attribute float a_inst_rot;
attribute float a_inst_scale;

varying lowp vec4 color;

void main() {
    float s = sin(a_inst_rot);
    float c = cos(a_inst_rot);
    mat2 rotation_mat = mat2(c, -s, s, c);

    vec2 rotated_pos = a_pos * rotation_mat;
    vec2 scaled_pos = rotated_pos * a_inst_scale;
    vec2 pos = scaled_pos + a_inst_pos + u_pan;
    gl_Position = vec4(pos * vec2(1.0, -1.0) * u_zoom, 0.0, 1.0);

    color = a_color;
}
"#;

    pub const FRAGMENT: &str = r#"#version 100

varying lowp vec4 color;

void main() {
    gl_FragColor = color;
}
"#;

    pub const META: ShaderMeta = ShaderMeta {
        images: &[],
        uniforms: UniformBlockLayout {
            uniforms: &[
                UniformDesc::new("u_zoom", UniformType::Float2),
                UniformDesc::new("u_pan", UniformType::Float2),
            ],
        },
    };

    #[repr(C)]
    #[derive(Debug)]
    pub struct Uniforms {
        pub zoom: (f32, f32),
        pub pan: (f32, f32),
    }
}

mod post_process_shader {
    use miniquad::graphics::*;

    pub const VERTEX: &str = r#"#version 100

attribute vec2 a_pos;
attribute vec2 a_uv;

uniform lowp vec2 u_resolution;

varying lowp vec2 v_texcoord;
varying lowp vec2 v_resolution;

// Precalculated for FXAA
varying vec2 v_rgbNW;
varying vec2 v_rgbNE;
varying vec2 v_rgbSW;
varying vec2 v_rgbSE;
varying vec2 v_rgbM;

void texcoords(vec2 fragCoord, vec2 resolution,
			out vec2 v_rgbNW, out vec2 v_rgbNE,
			out vec2 v_rgbSW, out vec2 v_rgbSE,
			out vec2 v_rgbM) {
	vec2 inverseVP = 1.0 / resolution.xy;
	v_rgbNW = (fragCoord + vec2(-1.0, -1.0)) * inverseVP;
	v_rgbNE = (fragCoord + vec2(1.0, -1.0)) * inverseVP;
	v_rgbSW = (fragCoord + vec2(-1.0, 1.0)) * inverseVP;
	v_rgbSE = (fragCoord + vec2(1.0, 1.0)) * inverseVP;
	v_rgbM = vec2(fragCoord * inverseVP);
}

void main() {
    // Calculate the texture coordinates for the FXAA shader
    vec2 frag_coord = a_uv * u_resolution;
    texcoords(frag_coord, u_resolution, v_rgbNW, v_rgbNE, v_rgbSW, v_rgbSE, v_rgbM);

    gl_Position = vec4(a_pos, 0.0, 1.0);
    v_texcoord = a_uv;
    v_resolution = u_resolution;
}
"#;

    pub const FRAGMENT: &str = concat!(
        "#version 100\n",
        // The FXAA shader needs a defined precision
        "precision mediump float;\n",
        include_str!("fxaa.glsl"),
        r#"
varying lowp vec2 v_texcoord;
varying lowp vec2 v_resolution;

varying vec2 v_rgbNW;
varying vec2 v_rgbNE;
varying vec2 v_rgbSW;
varying vec2 v_rgbSE;
varying vec2 v_rgbM;

uniform sampler2D u_tex;

void main() {
    vec2 frag_coord = v_texcoord * v_resolution;
	gl_FragColor = fxaa(u_tex, frag_coord, v_resolution, v_rgbNW, v_rgbNE, v_rgbSW, v_rgbSE, v_rgbM);
}
"#
    );

    pub const META: ShaderMeta = ShaderMeta {
        images: &["u_tex"],
        uniforms: UniformBlockLayout {
            uniforms: &[UniformDesc::new("u_resolution", UniformType::Float2)],
        },
    };

    #[repr(C)]
    #[derive(Debug)]
    pub struct Uniforms {
        pub resolution: (f32, f32),
    }
}
