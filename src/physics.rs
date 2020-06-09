use crate::{render::Instance, Float};
use generational_arena::Index;
use legion::{
    query::{IntoQuery, Read, Write},
    schedule::Schedulable,
    system::SystemBuilder,
};
use nalgebra::{convert as f, RealField, Vector2};
use ncollide2d::shape::{Shape, ShapeHandle};
use nphysics2d::{
    force_generator::DefaultForceGeneratorSet,
    joint::DefaultJointConstraintSet,
    material::{BasicMaterial, MaterialHandle},
    object::{
        Body, BodyPartHandle, BodyStatus, ColliderDesc, DefaultBodyHandle, DefaultBodySet,
        DefaultColliderSet, RigidBodyDesc,
    },
    world::{DefaultGeometricalWorld, DefaultMechanicalWorld},
};

/// Physics world.
pub struct Physics<N: RealField> {
    mechanical_world: DefaultMechanicalWorld<N>,
    geometrical_world: DefaultGeometricalWorld<N>,

    bodies: DefaultBodySet<N>,
    colliders: DefaultColliderSet<N>,
    joint_constraints: DefaultJointConstraintSet<N>,
    force_generators: DefaultForceGeneratorSet<N>,
}

impl<N: RealField> Physics<N> {
    /// Instantiate the physics world.
    pub fn new(gravity: N) -> Self {
        let mechanical_world = DefaultMechanicalWorld::new(Vector2::new(nalgebra::zero(), gravity));

        Self {
            mechanical_world,
            geometrical_world: DefaultGeometricalWorld::new(),
            bodies: DefaultBodySet::new(),
            colliders: DefaultColliderSet::new(),
            joint_constraints: DefaultJointConstraintSet::new(),
            force_generators: DefaultForceGeneratorSet::new(),
        }
    }

    /// Run the simulation.
    pub fn step(&mut self) {
        self.mechanical_world.step(
            &mut self.geometrical_world,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.joint_constraints,
            &mut self.force_generators,
        );
    }

    /// Spawn a rigid body.
    pub fn spawn_rigid_body(
        &mut self,
        rigid_body_builder: &RigidBodyDesc<N>,
        collider_builder: &ColliderDesc<N>,
    ) -> RigidBody {
        let rigid_body = rigid_body_builder.build();
        let body_index = self.bodies.insert(rigid_body);

        let collider = collider_builder.build(BodyPartHandle(body_index, 0));
        let collider_index = self.colliders.insert(collider);

        RigidBody {
            body_index,
            collider_index,
        }
    }

    /// Spawn a body.
    pub fn spawn_body<B>(&mut self, body: B, collider_builder: &ColliderDesc<N>) -> RigidBody
    where
        B: Body<N>,
    {
        let body_index = self.bodies.insert(body);

        let collider = collider_builder.build(BodyPartHandle(body_index, 0));
        let collider_index = self.colliders.insert(collider);

        RigidBody {
            body_index,
            collider_index,
        }
    }

    /// Get the position (with rotation) of a rigid body.
    pub fn position(&self, rigid_body: &RigidBody) -> Option<(N, N, N)> {
        self.bodies.rigid_body(rigid_body.body_index).map(|body| {
            let position = body.position();
            let translation = position.translation;

            (translation.x, translation.y, position.rotation.angle())
        })
    }

    /// Helps making constructing rigid bodies easier.
    pub fn default_rigid_body_builder() -> RigidBodyDesc<N> {
        RigidBodyDesc::new()
            .gravity_enabled(true)
            .status(BodyStatus::Dynamic)
            .linear_damping(f(0.1))
        //.angular_damping(f(0.0))
        //.max_linear_velocity(f(200.0))
        //.max_angular_velocity(f(1.7))
        //.angular_inertia(f(3.0))
        //.local_center_of_mass(Point2::new(f(1.0), f(1.0)))
    }

    /// Helps making constructing collision objects for rigid bodies easier.
    pub fn default_collider_builder<S: Shape<N>>(shape: S) -> ColliderDesc<N> {
        ColliderDesc::new(ShapeHandle::new(shape))
            .margin(f(0.1))
            .density(f(0.2))
            .material(MaterialHandle::new(BasicMaterial::new(f(0.1), f(0.5))))
    }

    /// Get the system for updating the render instance positions.
    pub fn render_system() -> Box<dyn Schedulable> {
        SystemBuilder::new("update_positions")
            .read_resource::<Physics<Float>>()
            .with_query(<(Write<Instance>, Read<RigidBody>)>::query())
            .build(|_, mut world, physics, query| {
                for (mut instance, rigid_body) in query.iter(&mut world) {
                    if let Some((x, y, rotation)) = physics.position(&rigid_body) {
                        instance.set_x(x as f32);
                        instance.set_y(y as f32);
                        instance.set_rotation(rotation as f32);
                    }
                }
            })
    }
}

/// A rigid body component.
pub struct RigidBody {
    body_index: DefaultBodyHandle,
    collider_index: Index,
}
