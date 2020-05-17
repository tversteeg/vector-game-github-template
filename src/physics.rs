use generational_arena::Index;
use nalgebra::convert as f;
use nalgebra::{Point2, RealField, Vector2};
use ncollide2d::shape::{Shape, ShapeHandle};
use nphysics2d::{
    force_generator::DefaultForceGeneratorSet,
    joint::DefaultJointConstraintSet,
    material::{BasicMaterial, MaterialHandle},
    math::Velocity,
    object::{
        BodyPartHandle, BodyStatus, ColliderDesc, DefaultBodyHandle, DefaultBodySet,
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
        Self {
            mechanical_world: DefaultMechanicalWorld::new(Vector2::new(nalgebra::zero(), gravity)),
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
        let rigid_body_index = self.bodies.insert(rigid_body);

        let collider = collider_builder.build(BodyPartHandle(rigid_body_index, 0));
        let collider_index = self.colliders.insert(collider);

        RigidBody {
            rigid_body_index,
            collider_index,
        }
    }

    /// Get the position of a rigid body.
    pub fn position(&self, rigid_body: &RigidBody) -> Option<(N, N)> {
        self.bodies
            .rigid_body(rigid_body.rigid_body_index)
            .map(|body| {
                let translation = body.position().translation;
                (translation.x, translation.y)
            })
    }

    /// Get all the positions of all objects.
    pub fn positions(&self) -> Vec<(N, N)> {
        self.colliders
            .iter()
            //.filter(|(_, collider)| collider.shape() == shape)
            .map(|(_, collider)| {
                let translation = collider.position().translation;
                (translation.x, translation.y)
            })
            .collect()
    }

    /// Helps making constructing rigid bodies easier.
    pub fn default_rigid_body_builder(
        position: Vector2<N>,
        velocity: Velocity<N>,
    ) -> RigidBodyDesc<N> {
        RigidBodyDesc::new()
            .translation(position)
            .rotation(nalgebra::zero())
            .gravity_enabled(true)
            .status(BodyStatus::Dynamic)
            .velocity(velocity)
            .linear_damping(f(0.0))
            .angular_damping(f(0.0))
            .max_linear_velocity(f(50.0))
            .max_angular_velocity(f(1.7))
            .angular_inertia(f(3.0))
            .mass(f(10.0))
            .local_center_of_mass(Point2::new(f(1.0), f(1.0)))
    }

    /// Helps making constructing collision objects for rigid bodies easier.
    pub fn default_collider_builder<S: Shape<N>>(shape: S) -> ColliderDesc<N> {
        ColliderDesc::new(ShapeHandle::new(shape))
            .density(f(1.3))
            .material(MaterialHandle::new(BasicMaterial::new(f(0.3), f(0.8))))
    }
}

/// A rigid body component.
pub struct RigidBody {
    rigid_body_index: DefaultBodyHandle,
    collider_index: Index,
}
