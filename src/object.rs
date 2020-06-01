use crate::{
    physics::{Physics, RigidBody},
    render::{Instance, Mesh},
};
use nalgebra::{RealField, Vector2};
use nphysics2d::object::{ColliderDesc, Ground, RigidBodyDesc};

/// Definition that can be used to spawn objects.
///
/// Objects contain a mesh to render and a rigid body and a collider for physics.
pub struct ObjectDef<N: RealField> {
    /// Mesh reference to render the object.
    pub mesh: Mesh,
    /// Description of the rigid body (not applicable when ground).
    pub rigid_body: RigidBodyDesc<N>,
    /// Description of the collision body.
    pub collider: ColliderDesc<N>,
    /// Whether the object is ground.
    pub is_ground: bool,
}

impl<N: RealField> ObjectDef<N> {
    /// Spawn a instance of this object which can be added to the ECS system.
    pub fn spawn(&self, physics: &mut Physics<N>, pos: Vector2<N>, z: u8) -> (Instance, RigidBody) {
        let mut instance = Instance::new(0.0, 0.0);
        instance.set_z(z);

        if self.is_ground {
            (instance, physics.spawn_body(Ground::new(), &self.collider))
        } else {
            (instance, self.spawn_rigid_body(physics, pos))
        }
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
