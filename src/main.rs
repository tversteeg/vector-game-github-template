mod object;
mod physics;
mod render;
mod svg;

use crate::{
    object::ObjectDef,
    physics::{Physics, RigidBody},
    render::{Instance, Render},
    svg::Svg,
};
use anyhow::Result;
use legion::{
    query::{IntoQuery, Read, Write},
    schedule::Schedule,
    system::SystemBuilder,
    world::{Universe, World},
};
use miniquad::{
    conf::{Conf, Loading},
    Context, EventHandler, UserData,
};

type Float = f64;
type Vec2 = nalgebra::Vector2<Float>;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;

const ZOOM_FACTOR: f32 = 30.0;
const MAX_ZOOM: f32 = 20.0;

/// Our game state.
struct Game {
    /// Our wrapper around the OpenGL calls.
    render: Render,
    /// ECS world.
    world: World,
    /// ECS schedule for the systems.
    schedule: Schedule,
    /// The camera zoom value.
    zoom: f32,
    /// The object definition for arrows.
    arrow_def: ObjectDef,
}

impl Game {
    /// Setup the ECS and load the systems.
    pub fn new(ctx: &mut Context) -> Result<Self> {
        // Setup the OpenGL render part
        let mut render = Render::new(ctx);

        // Parse SVG and convert it to object definitions
        let mut character_def = Svg::from_str(include_str!("../assets/single-character.svg"))?
            .into_object_def(&mut render)?;
        let mut ground_def =
            Svg::from_str(include_str!("../assets/ground.svg"))?.into_object_def(&mut render)?;
        let arrow_def =
            Svg::from_str(include_str!("../assets/arrow.svg"))?.into_object_def(&mut render)?;

        // Instantiate the physics engine
        let mut physics = Physics::new(9.81 * 100.0);

        // Instantiate the ECS
        let universe = Universe::new();
        let mut world = universe.create_world();

        // Add the ground
        world.insert(
            (ground_def.mesh(),),
            vec![ground_def.spawn(&mut physics, Vec2::new(-2000.0, 0.0), 0)],
        );

        // Add characters with rigid bodies
        world.insert(
            (character_def.mesh(),),
            (0..10).map(|i| {
                character_def.spawn(
                    &mut physics,
                    Vec2::new((i * 20) as f64, (-i * 100) as f64),
                    1,
                )
            }),
        );

        // Setup the ECS resources with the physics system
        world.resources.insert(physics);

        // Create the system for updating the instance positions
        let update_positions = SystemBuilder::new("update_positions")
            .read_resource::<Physics<f64>>()
            .with_query(<(Write<Instance>, Read<RigidBody>)>::query())
            .build(|_, mut world, physics, query| {
                for (mut instance, rigid_body) in query.iter(&mut world) {
                    if let Some((x, y, rotation)) = physics.position(&rigid_body) {
                        instance.set_x(x as f32);
                        instance.set_y(y as f32);
                        instance.set_rotation(rotation as f32);
                    }
                }
            });

        let schedule = Schedule::builder()
            .add_system(update_positions)
            .flush()
            .build();

        Ok(Self {
            render,
            world,
            schedule,
            zoom: 0.0,
            arrow_def,
        })
    }
}

impl EventHandler for Game {
    fn update(&mut self, _ctx: &mut Context) {
        // Move the physics
        {
            let mut physics = self.world.resources.get_mut::<Physics<f64>>().unwrap();
            physics.step();
        }

        // Run the systems scheduler
        self.schedule.execute(&mut self.world);

        self.render.update(&mut self.world);
    }

    fn draw(&mut self, ctx: &mut Context) {
        // Render the buffer
        self.render.render(ctx);
    }

    fn mouse_motion_event(&mut self, _ctx: &mut Context, x: f32, y: f32) {
        self.render.set_camera_pos(-x, -y);
    }

    fn mouse_wheel_event(&mut self, _ctx: &mut Context, _x: f32, y: f32) {
        self.zoom += y;
        self.zoom = self.zoom.max(-MAX_ZOOM).min(MAX_ZOOM);

        self.render.set_camera_zoom(1.0 + (self.zoom / ZOOM_FACTOR));
    }
}

fn main() {
    miniquad::start(
        Conf {
            window_title: concat!("replace_me - ", env!("CARGO_PKG_VERSION")).to_string(),
            window_width: WIDTH as i32,
            window_height: HEIGHT as i32,
            loading: Loading::Embedded,
            sample_count: 8,
            ..Default::default()
        },
        |mut ctx| {
            UserData::owning(
                Game::new(&mut ctx).expect("Setting up game state failed"),
                ctx,
            )
        },
    );
}
