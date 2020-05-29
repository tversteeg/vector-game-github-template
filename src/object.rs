use crate::{render::{Mesh, Instance}, physics::{Physics, RigidBody}};
use nphysics2d::object::{RigidBodyDesc, ColliderDesc};
use nalgebra::{Vector2, RealField};

/// Definition that can be used to spawn objects.
///
/// Objects contain a mesh to render and a rigid body and a collider for physics.
pub struct ObjectDef<N: RealField> {
    pub mesh: Mesh,
    pub rigid_body: RigidBodyDesc<N>,
    pub collider: ColliderDesc<N>
}

impl<N: RealField> ObjectDef<N> {
    /// Spawn a instance of this object which can be added to the ECS system.
    pub fn spawn(&self, physics: &mut Physics<N>, pos: Vector2<N>) -> (Instance, RigidBody) {
        (Instance::new(0.0, 0.0),

        self.spawn_rigid_body(physics, pos))
    }

    /// Spawn a rigid body in the physics system.
    pub fn spawn_rigid_body(&self, physics: &mut Physics<N>, pos: Vector2<N>) -> RigidBody {
        let rigid_body = self.rigid_body.clone().translation(pos);

        physics.spawn_rigid_body(&rigid_body, &self.collider)
    }

    /// Get the mesh reference.
    pub fn mesh(&self) -> Mesh {
        self.mesh
    }
}
