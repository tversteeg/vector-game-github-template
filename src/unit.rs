use crate::{object::ObjectDef, physics::Physics, Float, Vec2};
use legion::world::World;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Allegiance {
    Enemy,
    Ally,
}

impl Default for Allegiance {
    fn default() -> Self {
        Self::Enemy
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Health(Float);

impl Default for Health {
    fn default() -> Self {
        Self(100.0)
    }
}

pub struct UnitBuilder<'a> {
    allegiance: Allegiance,
    health: Health,
    pos: Vec2,
    z: u8,
    def: &'a mut ObjectDef,
}

impl<'a> UnitBuilder<'a> {
    /// Create a default ally unit.
    pub fn ally(def: &'a mut ObjectDef) -> Self {
        Self {
            allegiance: Allegiance::Ally,
            def,
            pos: Vec2::default(),
            z: 0,
            health: Health::default(),
        }
    }

    /// Create a default enemy unit.
    pub fn enemy(def: &'a mut ObjectDef) -> Self {
        Self {
            allegiance: Allegiance::Enemy,
            def,
            pos: Vec2::default(),
            z: 0,
            health: Health::default(),
        }
    }

    /// Spawn the unit in the world.
    pub fn spawn(self, world: &mut World, physics: &mut Physics<Float>) {
        let (instance, rigid_body) = self.def.spawn(physics, self.pos, self.z);

        world.insert(
            (self.def.mesh(),),
            vec![(instance, rigid_body, self.health, self.allegiance)],
        );
    }

    /// Set the lifepoints of the unit.
    pub fn health(mut self, health: Float) -> Self {
        self.health = Health(health);

        self
    }

    /// Set the position of the unit.
    pub fn pos(mut self, x: Float, y: Float) -> Self {
        self.pos = Vec2::new(x, y);

        self
    }

    /// Set the z index of the unit.
    pub fn z(mut self, z: u8) -> Self {
        self.z = z;

        self
    }
}
