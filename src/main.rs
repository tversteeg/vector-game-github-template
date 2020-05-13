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

/// Our game state.
struct Game {
    /// Our wrapper around the OpenGL calls.
    render: Render,
    /// Physics engine.
    physics: Physics<f64>,
    /// ECS universe.
    universe: Universe,
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

        // Instantiate the physics engine
        let mut physics = Physics::new(-9.81);

        // Instantiate the ECS
        let universe = Universe::new();
        let mut world = universe.create_world();

        // Add 10 characters with rigid bodies
        world.insert(
            (character_mesh,),
            (0..9).map(|x| {
                let rigid_body_desc = Physics::default_rigid_body_builder(
                    Vec2::new(x as f64, 0.0),
                    Velocity::linear(0.0, 0.0),
                );
                let collider_body_desc = Physics::default_collider_builder(Ball::new(1.5));
                (
                    Instance::new(x as f32 * 100.0, 0.0),
                    physics.spawn_rigid_body(&rigid_body_desc, &collider_body_desc),
                )
            }),
        );

        // Create the system for updating the instance positions
        let update_positions = SystemBuilder::<()>::new("update_positions")
            .with_query(<(Write<Instance>, Read<RigidBody>)>::query())
            .build(|_, mut world, _, query| {
                for (mut instance, rigid_body) in query.iter_mut(&mut world) {
                    instance.x += 1.0;
                }
            });

        let mut schedule = Schedule::builder()
            .add_system(update_positions)
            .flush()
            .build();

        Ok(Self {
            render,
            physics,
            universe,
            world,
            schedule,
        })
    }
}

impl EventHandler for Game {
    fn update(&mut self, _ctx: &mut Context) {
        // Move the physics
        self.physics.step();

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
