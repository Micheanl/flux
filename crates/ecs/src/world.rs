use std::any::{Any, TypeId};
use std::collections::HashMap;

use hecs::{
    Component, ComponentError, DynamicBundle, Entity, NoSuchEntity, Query, QueryBorrow, QueryMut,
};

#[derive(Default)]
struct Resources {
    entries: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Resources {
    fn insert<T: Any + Send + Sync>(&mut self, value: T) -> Option<T> {
        self.entries
            .insert(TypeId::of::<T>(), Box::new(value))
            .and_then(|previous| previous.downcast::<T>().ok())
            .map(|boxed| *boxed)
    }

    fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.entries
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref())
    }

    fn get_mut<T: Any + Send + Sync>(&mut self) -> Option<&mut T> {
        self.entries
            .get_mut(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_mut())
    }

    fn remove<T: Any + Send + Sync>(&mut self) -> Option<T> {
        self.entries
            .remove(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast::<T>().ok())
            .map(|boxed| *boxed)
    }
}

#[derive(Default)]
pub struct World {
    entities: hecs::World,
    resources: Resources,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(&mut self, components: impl DynamicBundle) -> Entity {
        self.entities.spawn(components)
    }

    pub fn despawn(&mut self, entity: Entity) -> bool {
        self.entities.despawn(entity).is_ok()
    }

    pub fn query<Q: Query>(&self) -> QueryBorrow<'_, Q> {
        self.entities.query()
    }

    pub fn query_mut<Q: Query>(&mut self) -> QueryMut<'_, Q> {
        self.entities.query_mut()
    }

    pub fn get<T: Component>(&self, entity: Entity) -> Option<hecs::Ref<'_, T>> {
        self.entities.get::<&T>(entity).ok()
    }

    pub fn get_mut<T: Component>(&self, entity: Entity) -> Option<hecs::RefMut<'_, T>> {
        self.entities.get::<&mut T>(entity).ok()
    }

    pub fn insert<T: Component>(
        &mut self,
        entity: Entity,
        component: T,
    ) -> Result<(), NoSuchEntity> {
        self.entities.insert_one(entity, component)
    }

    pub fn remove<T: Component>(&mut self, entity: Entity) -> Result<T, ComponentError> {
        self.entities.remove_one::<T>(entity)
    }

    pub fn insert_resource<T: Any + Send + Sync>(&mut self, value: T) -> Option<T> {
        self.resources.insert(value)
    }

    pub fn resource<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.resources.get()
    }

    pub fn resource_mut<T: Any + Send + Sync>(&mut self) -> Option<&mut T> {
        self.resources.get_mut()
    }

    pub fn expect_resource<T: Any + Send + Sync>(&self) -> &T {
        self.resource::<T>()
            .unwrap_or_else(|| panic!("resource {} is not registered", std::any::type_name::<T>()))
    }

    pub fn expect_resource_mut<T: Any + Send + Sync>(&mut self) -> &mut T {
        self.resource_mut::<T>()
            .unwrap_or_else(|| panic!("resource {} is not registered", std::any::type_name::<T>()))
    }

    pub fn remove_resource<T: Any + Send + Sync>(&mut self) -> Option<T> {
        self.resources.remove()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Health(u32);

    #[test]
    fn spawn_query_despawn() {
        let mut world = World::new();
        let entity = world.spawn((Health(10), 42u32));
        assert_eq!(world.get::<Health>(entity).map(|health| health.0), Some(10));
        assert_eq!(world.query::<(&Health, &u32)>().iter().count(), 1);
        assert!(world.despawn(entity));
    }

    #[test]
    fn resources_round_trip() {
        let mut world = World::new();
        assert!(world.insert_resource(7u32).is_none());
        assert_eq!(world.resource::<u32>(), Some(&7));
        *world.resource_mut::<u32>().unwrap() = 9;
        assert_eq!(world.remove_resource::<u32>(), Some(9));
        assert!(world.resource::<u32>().is_none());
    }
}
