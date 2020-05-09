use anyhow::Result;
use legion::{
    filter::filter_fns::tag_value,
    query::{IntoQuery, Read, Tagged},
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

type Vec2 = nalgebra::Vector2<f64>;

const MAX_MESH_INSTANCES: usize = 1024 * 1024;

/// A reference to an uploaded vector path.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Mesh(usize);

/// A wrapper around the OpenGL calls so the main file won't be polluted.
pub struct Render {
    pipeline: Pipeline,
    /// A list of draw calls with bindings that will be generated.
    draw_calls: Vec<DrawCall>,
    /// Whether some draw calls are missing bindings.
    missing_bindings: bool,
}

impl Render {
    /// Setup the OpenGL pipeline and the texture for the framebuffer.
    pub fn new(ctx: &mut Context) -> Self {
        // Create an OpenGL pipeline
        let shader = Shader::new(ctx, shader::VERTEX, shader::FRAGMENT, shader::META);
        let pipeline = Pipeline::new(
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
            ],
            shader,
        );

        Self {
            pipeline,
            draw_calls: vec![],
            missing_bindings: false,
        }
    }

    /// Upload a lyon path.
    ///
    /// Returns a reference that can be used to add instances.
    pub fn upload_path<P>(&mut self, path: P, color: usvg::Color, opacity: f32) -> Mesh
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
            instance_positions: vec![],
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

        let rtree = usvg::Tree::from_str(svg.as_ref(), &usvg::Options::default())?;
        // Loop over all nodes in the SVG tree
        for node in rtree.root().descendants() {
            if let usvg::NodeKind::Path(ref path) = *node.borrow() {
                if let Some(ref fill) = path.fill {
                    // Get the fill color
                    let color = match fill.paint {
                        usvg::Paint::Color(color) => color,
                        _ => todo!("Color not defined"),
                    };

                    // Tessellate the fill
                    fill_tess
                        .tessellate(
                            convert_path(path),
                            &FillOptions::tolerance(0.1),
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
                        &stroke_opts.with_tolerance(0.1),
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
            instance_positions: vec![],
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
                draw_call.instances = query.iter(world).map(|pos| *pos).collect();

                // Tell the render loop that the position of the instances have been changed
                draw_call.refresh_instances = true;
            });
    }

    /// Render the graphics.
    pub fn render(&mut self, ctx: &mut Context) {
        let (width, height) = ctx.screen_size();

        // Create bindings & update the instance vertices if necessary
        if self.missing_bindings {
            self.draw_calls.iter_mut().for_each(|mut dc| {
                // Create bindings if missing
                if dc.bindings.is_none() {
                    dc.create_bindings(ctx);
                }

                if dc.refresh_instances {
                    // Upload the instance positions
                    let bindings = dc.bindings.as_ref().unwrap();
                    bindings.vertex_buffers[1].update(ctx, &dc.instances);

                    dc.refresh_instances = false;
                }
            });

            self.missing_bindings = false;
        }

        // Start rendering
        ctx.begin_default_pass(PassAction::Nothing);

        // Render the separate draw calls
        for dc in self.draw_calls.iter_mut() {
            // Only render when we actually have instances
            if dc.instances.is_empty() {
                continue;
            }

            let bindings = dc.bindings.as_ref().unwrap();

            ctx.apply_pipeline(&self.pipeline);
            ctx.apply_scissor_rect(0, 0, width as i32, height as i32);
            ctx.apply_bindings(bindings);
            ctx.apply_uniforms(&Uniforms {
                zoom: (2.0 / width, 2.0 / height),
                pan: (-width / 2.0, -height / 2.0),
            });
            ctx.draw(0, dc.indices.len() as i32, dc.instances.len() as i32);
        }

        ctx.end_render_pass();

        ctx.commit_frame();
    }
}

/// A single uploaded mesh as a draw call.
#[derive(Debug)]
struct DrawCall {
    /// Render vertices, build by lyon path.
    vertices: Vec<Vertex>,
    /// Render indices, build by lyon path.
    indices: Vec<u16>,
    /// Position data for the instances.
    instance_positions: Vec<[f32; 2]>,
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

    /// Clear the list of instances.
    fn clear_instances(&mut self) {
        self.instances.clear();
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
struct Vertex {
    pos: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Debug)]
struct Uniforms {
    zoom: (f32, f32),
    pan: (f32, f32),
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Instance {
    position: [f32; 2],
}

impl Instance {
    /// Create a new instance with a position.
    pub fn new(x: f32, y: f32) -> Self {
        Self { position: [x, y] }
    }
}

/// Used by lyon to create vertices.
struct VertexCtor {
    color: [f32; 4],
}

impl VertexCtor {
    pub fn new(color: usvg::Color, alpha: f32) -> Self {
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

fn convert_stroke(s: &usvg::Stroke) -> (usvg::Color, StrokeOptions) {
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

    let opt = StrokeOptions::tolerance(0.01)
        .with_line_width(s.width.value() as f32)
        .with_line_cap(linecap)
        .with_line_join(linejoin);

    (color, opt)
}

mod shader {
    use miniquad::graphics::*;

    pub const VERTEX: &str = r#"#version 100

uniform vec2 u_zoom;
uniform vec2 u_pan;

attribute vec2 a_pos;
attribute vec4 a_color;
attribute vec2 a_inst_pos;

varying lowp vec4 color;

void main() {
    vec2 pos = a_pos + a_inst_pos + u_pan;
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
}
