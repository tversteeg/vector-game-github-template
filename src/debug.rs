use crate::{
    physics::Physics,
    render::{Instance, Mesh, Render},
};
use lyon::path::{
    math::{Angle, Point, Vector},
    Path,
};
use ncollide2d::shape::{Ball, Capsule, Cuboid};
use usvg::Color;

const MESH_COLOR: Color = Color {
    red: 0x00,
    green: 0x00,
    blue: 0xFF,
};

/// Render the physics shapes.
pub struct DebugPhysics {
    circle_mesh: Mesh,
    cuboid_mesh: Mesh,
}

impl DebugPhysics {
    /// Instantiate everything and upload the meshes.
    pub fn new(render: &mut Render) -> Self {
        let circle_mesh = Self::circle_mesh(render);
        let cuboid_mesh = Self::cuboid_mesh(render);

        Self {
            circle_mesh,
            cuboid_mesh,
        }
    }

    /// Render the debug shapes.
    pub fn render(&self, render: &mut Render, physics: &Physics<f64>) {
        let circles = physics
            .debug_shapes::<Ball<f64>>()
            .into_iter()
            .map(|(x, y, _, scale)| {
                let mut instance = Instance::new((x) as f32, (y) as f32);

                instance.set_scale(scale as f32);

                instance
            })
            .collect();
        render.set_instances(&self.circle_mesh, circles);

        let cuboids = physics
            .debug_shapes::<Cuboid<f64>>()
            .into_iter()
            .map(|(x, y, _, scale)| {
                let mut instance = Instance::new((x) as f32, (y) as f32);

                instance.set_scale(scale as f32);

                instance
            })
            .collect();
        render.set_instances(&self.cuboid_mesh, cuboids);
    }

    /// Upload the circle mesh.
    fn circle_mesh(render: &mut Render) -> Mesh {
        let mut builder = Path::builder();
        builder.move_to(Point::new(1.0, 0.5));
        builder.arc(
            Point::new(0.5, 0.5),
            Vector::new(0.5, 0.5),
            Angle::degrees(360.0),
            Angle::degrees(0.0),
        );

        render.upload_path(builder.build().iter(), MESH_COLOR, 0.5)
    }

    /// Upload the square mesh.
    fn cuboid_mesh(render: &mut Render) -> Mesh {
        let mut builder = Path::builder();
        builder.move_to(Point::new(1.0, 1.0));
        builder.line_to(Point::new(1.0, 0.0));
        builder.line_to(Point::new(0.0, 0.0));
        builder.line_to(Point::new(0.0, 1.0));
        builder.close();

        render.upload_path(builder.build().iter(), MESH_COLOR, 0.5)
    }
}
