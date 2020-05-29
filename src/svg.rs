use crate::render::{Vertex, VertexCtor};
use anyhow::{anyhow, Result};
use lyon::{
    math::Point,
    path::PathEvent,
    tessellation::{
        BuffersBuilder, FillOptions, FillTessellator, LineCap, LineJoin, StrokeOptions,
        StrokeTessellator, VertexBuffers,
    },
};
use usvg::{Color, NodeKind, Options, Paint, Path, PathSegment, ShapeRendering, Stroke, Tree};

const PATH_TOLERANCE: f32 = 0.01;

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

pub fn parse_svg<S>(svg: S) -> Result<VertexBuffers<Vertex, u16>>
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

fn convert_path<'a>(p: &'a Path) -> PathConvIter<'a> {
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
