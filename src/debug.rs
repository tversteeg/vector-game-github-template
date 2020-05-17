use crate::{
    physics::Physics,
    render::{Instance, Mesh, Render},
    PIXELS_PER_METER,
};
use lyon::path::{
    math::{Angle, Point, Vector},
    Path,
};
use nalgebra::RealField;
use usvg::Color;

const MESH_COLOR: Color = Color {
    red: 0x00,
    green: 0x00,
    blue: 0xFF,
};

/// Render the physics shapes.
pub struct DebugPhysics {
    circle_mesh: Mesh,
}

impl DebugPhysics {
    /// Instantiate everything and upload the meshes.
    pub fn new(render: &mut Render) -> Self {
        let circle_mesh = Self::circle_mesh(render);

        Self { circle_mesh }
    }

    /// Render the debug shapes.
    pub fn render(&self, render: &mut Render, physics: &Physics<f64>) {
        let circles = physics
            .positions()
            .into_iter()
            .map(|pos| {
                Instance::new(
                    (pos.0 * PIXELS_PER_METER) as f32,
                    (pos.1 * PIXELS_PER_METER) as f32,
                )
            })
            .collect();
        render.set_instances(&self.circle_mesh, circles);
    }

    /// Upload the circle mesh.
    fn circle_mesh(render: &mut Render) -> Mesh {
        let mut builder = Path::builder();
        builder.move_to(Point::new(1.0, 0.0));
        builder.arc(
            Point::new(0.0, 0.0),
            Vector::new(1.0, 1.0),
            Angle::degrees(360.0),
            Angle::degrees(0.0),
        );

        render.upload_path(builder.build().iter(), MESH_COLOR, 0.5)
    }
}
