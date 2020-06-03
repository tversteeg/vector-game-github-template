use crate::{
    object::ObjectDef,
    physics::Physics,
    render::{Mesh, Render, Vertex, VertexCtor},
};
use anyhow::{anyhow, Result};
use lyon::{
    math::Point,
    path::PathEvent,
    tessellation::{
        BuffersBuilder, FillOptions, FillTessellator, LineCap, LineJoin, StrokeOptions,
        StrokeTessellator, VertexBuffers,
    },
};
use nalgebra::{convert as f, Isometry2, RealField, Vector2};
use ncollide2d::shape::{Ball, Compound, Cuboid, ShapeHandle};
use std::borrow::Cow;
use usvg::{Color, NodeKind, Options, Paint, Path, PathSegment, ShapeRendering, Stroke, Tree};
use xmltree::Element;

const PATH_TOLERANCE: f32 = 0.01;

/// A parsed SVG containing the mesh and the specific metadata.
pub struct Svg {
    /// The lyon geometry.
    geometry: VertexBuffers<Vertex, u16>,
    /// The metadata XML node.
    metadata: Option<Element>,
}

impl Svg {
    /// Parse a SVG string.
    pub fn from_str(svg: &str) -> Result<Self> {
        // Simplify SVG
        let options = Options {
            shape_rendering: ShapeRendering::GeometricPrecision,
            keep_named_groups: false,
            ..Default::default()
        };
        let rtree = Tree::from_str(svg, &options)?;

        // Parse the SVG as XML to get the metadata
        let document = Element::parse(svg.as_bytes())?;
        let metadata = document.get_child("metadata").cloned();

        Ok(Self {
            geometry: parse_node(rtree)?,
            metadata,
        })
    }

    /// Upload it and get a mesh.
    pub fn upload(&self, render: &mut Render) -> Result<Mesh> {
        render.upload_buffers(&self.geometry)
    }

    /// Get the value of a metadata field.
    pub fn metadata(&self, key: &str) -> Option<Cow<str>> {
        self.metadata
            .as_ref()?
            .get_child(key)
            .map(|element| element.get_text())
            .flatten()
    }

    /// Build an object definition.
    ///
    /// Also upload the mesh.
    pub fn into_object_def(self, render: &mut Render) -> Result<ObjectDef> {
        let mesh = self.upload(render)?;

        let is_ground = self
            .metadata_collider_element()
            .ok_or_else(|| anyhow!("Metadata tag missing"))?
            .attributes
            .contains_key("ground");

        let rigid_body = Physics::default_rigid_body_builder();
        let collider = Physics::default_collider_builder(
            self.parse_metadata_colliders()
                .ok_or_else(|| anyhow!("Could not find colliders in shape"))?,
        );

        Ok(ObjectDef {
            is_ground,
            mesh,
            rigid_body,
            collider,
        })
    }

    /// Get the colliders from the SVG metadata.
    fn parse_metadata_colliders<N>(&self) -> Option<Compound<N>>
    where
        N: RealField,
    {
        // Get the colliders element in the metadata section
        let shapes = self
            .metadata_collider_element()?
            .children
            .iter()
            .map(|node| {
                let element = node.as_element().expect("Node is not a proper XML element");
                let (offset, shape_handle) = match element.name.as_str() {
                    // Parse an SVG circle element
                    "circle" => {
                        let offset_x = element.attributes["cx"]
                            .parse::<f64>()
                            .expect("Node is not a proper floating point");
                        let offset_y = element.attributes["cy"]
                            .parse::<f64>()
                            .expect("Node is not a proper floating point");
                        let offset = Vector2::new(f(offset_x), f(offset_y));

                        let radius = element.attributes["r"]
                            .parse::<f64>()
                            .expect("Node is not a proper floating point");
                        let shape = Ball::<N>::new(f(radius));

                        (offset, ShapeHandle::new(shape))
                    }
                    // Parse an SVG rectangle element
                    "rect" => {
                        let offset_x = element.attributes["x"]
                            .parse::<f64>()
                            .expect("Node is not a proper floating point");
                        let offset_y = element.attributes["y"]
                            .parse::<f64>()
                            .expect("Node is not a proper floating point");
                        let width = element.attributes["width"]
                            .parse::<f64>()
                            .expect("Node is not a proper floating point");
                        let height = element.attributes["height"]
                            .parse::<f64>()
                            .expect("Node is not a proper floating point");

                        let offset =
                            Vector2::new(f(offset_x + width / 2.0), f(offset_y + height / 2.0));

                        let shape = Cuboid::<N>::new(Vector2::new(f(width / 2.0), f(height / 2.0)));

                        (offset, ShapeHandle::new(shape))
                    }
                    other => panic!("Unrecognized metadata collider element \"{}\".", other),
                };

                (Isometry2::new(offset, nalgebra::zero()), shape_handle)
            })
            .collect::<Vec<_>>();

        if !shapes.is_empty() {
            Some(Compound::new(shapes))
        } else {
            None
        }
    }

    /// Get the collider element.
    pub fn metadata_collider_element(&self) -> Option<&Element> {
        let metadata = self.metadata.as_ref()?;

        metadata
            .get_child("colliders")
            .or_else(|| metadata.get_child("collider"))
    }
}

struct PathConvIter<'a> {
    iter: std::slice::Iter<'a, PathSegment>,
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
            Some(PathSegment::MoveTo { x, y }) => {
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
            Some(PathSegment::LineTo { x, y }) => {
                self.needs_end = true;
                let from = self.prev;
                self.prev = point(x, y);
                Some(PathEvent::Line {
                    from,
                    to: self.prev,
                })
            }
            Some(PathSegment::CurveTo {
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
            Some(PathSegment::ClosePath) => {
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

fn parse_node(rtree: Tree) -> Result<VertexBuffers<Vertex, u16>> {
    // Tessalate the path, converting it to vertices & indices
    let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();

    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();

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
                    .map_err(|err| anyhow!("tesselation failed: {:?}", err))?;
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

    Ok(geometry)
}

fn point(x: &f64, y: &f64) -> Point {
    Point::new((*x) as f32, (*y) as f32)
}

fn convert_path(p: &Path) -> PathConvIter {
    PathConvIter {
        iter: p.data.iter(),
        first: Point::new(0.0, 0.0),
        prev: Point::new(0.0, 0.0),
        deferred: None,
        needs_end: false,
    }
}

fn convert_stroke(s: &Stroke) -> (Color, StrokeOptions) {
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
