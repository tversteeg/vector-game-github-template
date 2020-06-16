mod object;
mod physics;
mod render;
mod svg;
mod text;
mod unit;

use crate::{
    object::ObjectDef, physics::Physics, render::Render, svg::Svg, text::Font, unit::UnitBuilder,
};
use anyhow::Result;
use glsp::{GFn, Lib, Root, Runtime, Val};
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
    /// The camera zoom value.
    zoom: f32,
    /// The object definition for arrows.
    arrow_def: ObjectDef,
    /// The scripting runtime.
    runtime: Runtime,
    /// The physics system.
    physics: Physics<Float>,
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

        // Parse a font
        let font = Font::from_bytes(include_bytes!("../assets/FetteNationalFraktur.ttf"))?.upload(
            &mut render,
            "ABCDEFGHIJKLMOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789".chars(),
        )?;

        // Instantiate the physics engine
        let mut physics = Physics::new(9.81 * 100.0);

        // Setup the script runtime
        let runtime = Runtime::new();
        runtime.run(|| {
            glsp::add_lib(render);

            glsp::eval_multi(
                &glsp::parse_all(include_str!("../scripts/main.glsp"), None)?,
                None,
            )?;

            Ok(())
        });

        Ok(Self {
            physics,
            zoom: 0.0,
            arrow_def,
            runtime,
        })
    }

    /// Run a GameLisp function.
    pub fn call(&self, function: &str) -> bool {
        struct RuntimeResult(bool);

        let result: RuntimeResult = self
            .runtime
            .run(|| {
                let update_func: Root<GFn> = match glsp::global(function) {
                    Ok(Val::GFn(update)) => update,
                    Ok(val) => {
                        eprintln!("invalid {} function: {}", function, val);

                        return Ok(RuntimeResult(false));
                    }
                    Err(err) => {
                        eprintln!("error finding {} function: {}", function, err);

                        return Ok(RuntimeResult(false));
                    }
                };
                let _: Val = glsp::call(&update_func, &())?;

                Ok(RuntimeResult(true))
            })
            .expect("Something unexpected went wrong with calling a GameLisp function");

        result.0
    }
}

impl EventHandler for Game {
    fn update(&mut self, ctx: &mut Context) {
        // Move the physics
        self.physics.step();

        // Call the update function in the main script
        if !self.call("engine:update") {
            ctx.request_quit();
        }
    }

    fn draw(&mut self, ctx: &mut Context) {
        self.runtime.run(|| {
            // Render the buffer
            Render::borrow_mut().render(ctx);

            Ok(())
        });

        // Call the render function in the main script
        if !self.call("engine:render") {
            ctx.request_quit();
        }
    }

    fn mouse_motion_event(&mut self, _ctx: &mut Context, x: f32, y: f32) {
        self.runtime.run(|| {
            // Set the camera position
            Render::borrow_mut().set_camera_pos(-x, -y);

            Ok(())
        });
    }

    fn mouse_wheel_event(&mut self, _ctx: &mut Context, _x: f32, y: f32) {
        self.zoom += y;
        self.zoom = self.zoom.max(-MAX_ZOOM).min(MAX_ZOOM);
        let zoom = 1.0 + (self.zoom / ZOOM_FACTOR);

        self.runtime.run(|| {
            // Set the camera zoom
            Render::borrow_mut().set_camera_zoom(zoom);

            Ok(())
        });
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
