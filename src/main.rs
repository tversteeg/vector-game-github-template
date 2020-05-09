mod physics;
mod render;

use crate::{
    physics::Physics,
    render::{Instance, Render},
};
use anyhow::Result;
use legion::world::{Universe, World};
use miniquad::{
    conf::{Conf, Loading},
    Context, EventHandler, UserData,
};

type Vec2 = nalgebra::Vector2<f64>;

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
}

impl Game {
    /// Setup the ECS and load the systems.
    pub fn new(ctx: &mut Context) -> Result<Self> {
        // Setup the OpenGL render part
        let mut render = Render::new(ctx);

        // Add a SVG
        let character_mesh = render.upload_svg(include_str!("../assets/single-character.svg"))?;

        // Instantiate the physics engine
        let physics = Physics::new(-9.81);

        // Instantiate the ECS
        let universe = Universe::new();
        let mut world = universe.create_world();

        // Add 10 characters
        world.insert(
            (character_mesh,),
            (0..9).map(|x| (Instance::new(x as f32 * 100.0, 0.0),)),
        );

        Ok(Self {
            render,
            physics,
            universe,
            world,
        })
    }
}

impl EventHandler for Game {
    fn update(&mut self, _ctx: &mut Context) {
        self.physics.step();

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
