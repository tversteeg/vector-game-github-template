mod physics;
mod render;

use crate::{
    physics::{Physics, RigidBody},
    render::{Instance, Render},
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
use ncollide2d::shape::Ball;

type Vec2 = nalgebra::Vector2<f64>;
type Velocity = nphysics2d::math::Velocity<f64>;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;

const PIXELS_PER_METER: f64 = 10.0;

/// Our game state.
struct Game {
    /// Our wrapper around the OpenGL calls.
    render: Render,
    /// ECS world.
    world: World,
    /// ECS schedule for the systems.
    schedule: Schedule,
}

impl Game {
    /// Setup the ECS and load the systems.
    pub fn new(ctx: &mut Context) -> Result<Self> {
        // Setup the OpenGL render part
        let mut render = Render::new(ctx);

        // Add a SVG
        let character_mesh = render.upload_svg(include_str!("../assets/single-character.svg"))?;
        let siege_tower_mesh = render.upload_svg(include_str!("../assets/siege-tower.svg"))?;

        // Instantiate the physics engine
        let mut physics = Physics::new(9.81);

        // Instantiate the ECS
        let universe = Universe::new();
        let mut world = universe.create_world();

        // Add 10 characters with rigid bodies
        world.insert(
            (character_mesh,),
            (0..9).map(|x| {
                let rigid_body_desc = Physics::default_rigid_body_builder(
                    Vec2::new(x as f64, 0.0),
                    Velocity::linear(x as f64, 0.0),
                );
                let collider_body_desc = Physics::default_collider_builder(Ball::new(1.0));
                (
                    Instance::new(0.0, 0.0),
                    physics.spawn_rigid_body(&rigid_body_desc, &collider_body_desc),
                )
            }),
        );

        // Add siege towers
        world.insert(
            (siege_tower_mesh,),
            (0..9).map(|x| {
                let rigid_body_desc = Physics::default_rigid_body_builder(
                    Vec2::new(x as f64, 0.0),
                    Velocity::linear(x as f64, 0.0),
                );
                let collider_body_desc = Physics::default_collider_builder(Ball::new(1.0));
                (
                    Instance::new(0.0, 0.0),
                    physics.spawn_rigid_body(&rigid_body_desc, &collider_body_desc),
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
                    let (x, y) = physics.position(&rigid_body).unwrap();
                    instance.set_x((x * PIXELS_PER_METER) as f32);
                    instance.set_y((y * PIXELS_PER_METER) as f32);
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
        })
    }
}

impl EventHandler for Game {
    fn update(&mut self, _ctx: &mut Context) {
        // Move the physics
        self.world
            .resources
            .get_mut::<Physics<f64>>()
            .unwrap()
            .step();

        // Run the systems scheduler
        self.schedule.execute(&mut self.world);

        self.render.update(&mut self.world);
    }

    fn draw(&mut self, ctx: &mut Context) {
        // Render the buffer
        self.render.render(ctx);
    }

    fn resize_event(&mut self, ctx: &mut Context, width: f32, height: f32) {
        self.render.resize(ctx, width, height);
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
