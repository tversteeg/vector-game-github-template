use generational_arena::Index;
use nalgebra::convert as f;
use nalgebra::{Point2, RealField, Vector2};
use ncollide2d::shape::{Ball, Capsule, Cuboid, Shape, ShapeHandle};
use nphysics2d::{
    force_generator::DefaultForceGeneratorSet,
    joint::DefaultJointConstraintSet,
    material::{BasicMaterial, MaterialHandle},
    object::{
        Body, BodyPartHandle, BodyStatus, ColliderDesc, DefaultBodyHandle, DefaultBodySet,
        DefaultColliderSet, Ground, RigidBodyDesc,
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

    /// Get all the positions (with rotation) of all objects.
    pub fn positions<S>(&self) -> Vec<(N, N, N)>
    where
        S: Shape<N>,
    {
        self.colliders
            .iter()
            .filter(|(_, collider)| collider.shape().as_shape::<S>().is_some())
            .map(|(_, collider)| {
                let position = collider.position();
                let translation = position.translation;

                (translation.x, translation.y, position.rotation.angle())
            })
            .collect()
    }

    /// Get the debug info of all objects.
    ///
    /// The fields in the tuple returned are: `X`, `Y`, `Rotation`, `Scale`.
    pub fn debug_shapes<S>(&self) -> Vec<(N, N, N, N)>
    where
        S: Shape<N> + ShapeSize<N>,
    {
        self.colliders
            .iter()
            .filter_map(|(_, collider)| {
                collider
                    .shape()
                    .as_shape::<S>()
                    .map(|shape| (collider, shape.size()))
            })
            .map(|(collider, size)| {
                let position = collider.position();
                let translation = position.translation;

                (
                    translation.x,
                    translation.y,
                    position.rotation.angle(),
                    size,
                )
            })
            .collect()
    }

    pub fn spawn_ground(&mut self, position: Vector2<N>, size: Vector2<N>) -> RigidBody {
        let ground_shape = ShapeHandle::new(Cuboid::new(size));
        let co = ColliderDesc::new(ground_shape).translation(position);

        self.spawn_body(Ground::new(), &co)
    }

    /// Helps making constructing rigid bodies easier.
    pub fn default_rigid_body_builder() -> RigidBodyDesc<N> {
        RigidBodyDesc::new()
            .rotation(nalgebra::zero())
            .gravity_enabled(true)
            .status(BodyStatus::Dynamic)
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
    body_index: DefaultBodyHandle,
    collider_index: Index,
}

/// A trait for getting the sizes of shapes.
pub trait ShapeSize<N: RealField> {
    fn size(&self) -> N;
}

impl<N: RealField> ShapeSize<N> for Capsule<N> {
    fn size(&self) -> N {
        self.radius()
    }
}

impl<N: RealField> ShapeSize<N> for Cuboid<N> {
    fn size(&self) -> N {
        self.half_extents().x * f(2.0)
    }
}

impl<N: RealField> ShapeSize<N> for Ball<N> {
    fn size(&self) -> N {
        self.radius()
    }
}
