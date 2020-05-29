mod debug;
mod physics;
mod render;
mod svg;

use crate::{
    debug::DebugPhysics,
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
use ncollide2d::shape::{Ball, Cuboid};

type Vec2 = nalgebra::Vector2<f64>;
type Velocity = nphysics2d::math::Velocity<f64>;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;

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
        let character_svg = Svg::from_str(include_str!("../assets/single-character.svg"))?;
        let character_mesh = character_svg.upload(&mut render)?;

        // Instantiate the physics engine
        let mut physics = Physics::new(9.81 * 10.0);

        // Instantiate the ECS
        let universe = Universe::new();
        let mut world = universe.create_world();

        // Add 10 characters with rigid bodies
        world.insert(
            (character_mesh,),
            (0..3).map(|x| {
                let rigid_body_desc = Physics::default_rigid_body_builder(
                    Vec2::new(x as f64, 0.0),
                    Velocity::linear(x as f64, 0.0),
                );
                let collider_body_desc = Physics::default_collider_builder(Cuboid::new(Vec2::new(75.0 / 2.0, 175.0 / 2.0)));
                (
                    Instance::new(0.0, 0.0),
                    physics.spawn_rigid_body(&rigid_body_desc, &collider_body_desc),
                )
            }),
        );

        // Add siege towers
        /*
        world.insert(
            (siege_tower_mesh,),
            (0..9).map(|x| {
                let rigid_body_desc = Physics::default_rigid_body_builder(
                    Vec2::new(x as f64, 0.0),
                    Velocity::linear(x as f64, 0.0),
                );
                let collider_body_desc = Physics::default_collider_builder(Ball::new(100.0));
                (
                    Instance::new(0.0, 0.0),
                    physics.spawn_rigid_body(&rigid_body_desc, &collider_body_desc),
                )
            }),
        );
        */

        // Add the ground
        physics.spawn_ground(Vec2::new(0.0, 50.0), Vec2::new(500.0, 20.0));

        // Setup the ECS resources with the physics system
        world.resources.insert(physics);

        // Render debug shapes for the physics
        world.resources.insert(DebugPhysics::new(&mut render));

        // Create the system for updating the instance positions
        let update_positions = SystemBuilder::new("update_positions")
            .read_resource::<Physics<f64>>()
            .with_query(<(Write<Instance>, Read<RigidBody>)>::query())
            .build(|_, mut world, physics, query| {
                for (mut instance, rigid_body) in query.iter(&mut world) {
                    let (x, y, rotation) = physics.position(&rigid_body).unwrap();
                    instance.set_x(x as f32);
                    instance.set_y(y as f32);
                    instance.set_rotation(rotation as f32);
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
        {
            let mut physics = self.world.resources.get_mut::<Physics<f64>>().unwrap();
            physics.step();

            let debug_physics = self.world.resources.get_mut::<DebugPhysics>().unwrap();
            debug_physics.render(&mut self.render, &physics);
        }

        // Run the systems scheduler
        self.schedule.execute(&mut self.world);

        self.render.update(&mut self.world);
    }

    fn draw(&mut self, ctx: &mut Context) {
        // Render the buffer
        self.render.render(ctx);
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
