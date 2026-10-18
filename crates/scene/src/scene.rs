use std::collections::HashMap;

use ecs::{Entity, World};
use foundation::{Aabb, Camera as ViewCamera, Mat4, Vec3};
use renderer::{
    CameraData, InstanceData, MaterialData, MeshData, PointLight as RenderPointLight, RenderScene,
    SunLight, TextureData,
};

use crate::components::{
    CameraComponent, Children, DirectionalLight, GlobalTransform, MeshRenderer, Name, Parent,
    PointLight, Transform,
};

#[derive(Default)]
pub struct Scene {
    world: World,
    render: RenderScene,
    active_camera: Option<Entity>,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn render(&self) -> &RenderScene {
        &self.render
    }

    pub fn render_mut(&mut self) -> &mut RenderScene {
        &mut self.render
    }

    pub fn add_mesh(&mut self, mesh: MeshData) -> u32 {
        self.render.add_mesh(mesh)
    }

    pub fn add_material(&mut self, material: MaterialData) -> u32 {
        self.render.add_material(material)
    }

    pub fn add_texture(&mut self, texture: TextureData) -> u32 {
        self.render.add_texture(texture)
    }

    pub fn spawn(&mut self, components: impl ecs::DynamicBundle) -> Entity {
        self.world.spawn(components)
    }

    pub fn despawn(&mut self, entity: Entity) -> bool {
        self.world.despawn(entity)
    }

    pub fn despawn_recursive(&mut self, entity: Entity) {
        let children = self
            .world
            .get::<Children>(entity)
            .map(|children| children.0.clone())
            .unwrap_or_default();
        for child in children {
            self.despawn_recursive(child);
        }
        self.despawn(entity);
    }

    pub fn spawn_mesh(&mut self, mesh: u32, material: u32, transform: Transform) -> Entity {
        self.spawn((
            transform,
            GlobalTransform(transform.matrix()),
            MeshRenderer::new(mesh, material),
        ))
    }

    pub fn spawn_camera(&mut self, transform: Transform, fov_y: f32, near: f32) -> Entity {
        let entity = self.spawn((
            transform,
            GlobalTransform(transform.matrix()),
            CameraComponent { fov_y, near },
        ));
        self.active_camera.get_or_insert(entity);
        entity
    }

    pub fn spawn_sun(&mut self, transform: Transform, light: DirectionalLight) -> Entity {
        self.spawn((transform, GlobalTransform(transform.matrix()), light))
    }

    pub fn spawn_point_light(&mut self, transform: Transform, light: PointLight) -> Entity {
        self.spawn((transform, GlobalTransform(transform.matrix()), light))
    }

    pub fn set_active_camera(&mut self, entity: Entity) {
        self.active_camera = Some(entity);
    }

    pub fn set_camera_transform(&mut self, entity: Entity, transform: Transform) {
        if let Some(mut slot) = self.world.get_mut::<Transform>(entity) {
            *slot = transform;
        }
    }

    pub fn set_name(&mut self, entity: Entity, name: &str) {
        let _ = self.world.insert(entity, Name(name.to_string()));
    }

    pub fn set_parent(&mut self, child: Entity, parent: Entity) {
        let _ = self.world.insert(child, Parent(parent));
        let attached = self
            .world
            .get_mut::<Children>(parent)
            .map(|mut children| {
                if !children.0.contains(&child) {
                    children.0.push(child);
                }
            })
            .is_some();
        if !attached {
            let _ = self.world.insert(parent, Children(vec![child]));
        }
    }

    pub fn extract(&mut self, aspect: f32) -> &RenderScene {
        self.update_hierarchy();
        self.extract_camera(aspect);
        self.extract_lights();
        self.extract_instances();
        &self.render
    }

    fn update_hierarchy(&mut self) {
        let nodes: Vec<(Entity, Mat4, Option<Entity>)> = self
            .world
            .query::<(Entity, &Transform, Option<&Parent>)>()
            .iter()
            .map(|(entity, transform, parent)| (entity, transform.matrix(), parent.map(|p| p.0)))
            .collect();
        if nodes.is_empty() {
            return;
        }

        let locals: HashMap<Entity, Mat4> = nodes
            .iter()
            .map(|(entity, local, _)| (*entity, *local))
            .collect();
        let mut children: HashMap<Entity, Vec<Entity>> = HashMap::new();
        let mut queue: Vec<(Entity, Mat4)> = Vec::new();
        for (entity, local, parent) in &nodes {
            match parent {
                Some(parent) => children.entry(*parent).or_default().push(*entity),
                None => queue.push((*entity, *local)),
            }
        }

        let mut globals: HashMap<Entity, Mat4> = HashMap::with_capacity(nodes.len());
        while let Some((entity, global)) = queue.pop() {
            globals.insert(entity, global);
            if let Some(kids) = children.get(&entity) {
                for kid in kids {
                    if let Some(local) = locals.get(kid) {
                        queue.push((*kid, global * *local));
                    }
                }
            }
        }

        for (entity, _, _) in &nodes {
            if let Some(global) = globals.get(entity) {
                let _ = self.world.insert(*entity, GlobalTransform(*global));
            }
        }
    }

    fn extract_camera(&mut self, aspect: f32) {
        let Some(entity) = self.active_camera else {
            return;
        };
        let Some(camera) = self
            .world
            .get::<CameraComponent>(entity)
            .map(|camera| *camera)
        else {
            return;
        };
        let Some(global) = self
            .world
            .get::<GlobalTransform>(entity)
            .map(|global| *global)
        else {
            return;
        };
        let (_, rotation, _) = global.0.to_scale_rotation_translation();
        let transform = Transform {
            translation: global.translation(),
            rotation,
            scale: Vec3::ONE,
        };
        let view = ViewCamera::perspective(&transform, camera.fov_y, aspect.max(0.01), camera.near);
        self.render.camera = CameraData::from_camera(&view);
    }

    fn extract_lights(&mut self) {
        let sun = self
            .world
            .query::<(&DirectionalLight, &GlobalTransform)>()
            .iter()
            .next()
            .map(|(light, global)| SunLight {
                direction: -global.forward(),
                color: light.color,
                intensity: light.intensity,
            });
        if let Some(sun) = sun {
            self.render.sun = sun;
        }
        self.render.point_lights = self
            .world
            .query::<(&PointLight, &GlobalTransform)>()
            .iter()
            .map(|(light, global)| RenderPointLight {
                position: global.translation(),
                color: light.color,
                intensity: light.intensity,
                radius: light.radius,
            })
            .collect();
    }

    fn extract_instances(&mut self) {
        let mut instances = Vec::new();
        {
            let render = &self.render;
            for (mesh_renderer, global) in self
                .world
                .query::<(&MeshRenderer, &GlobalTransform)>()
                .iter()
            {
                let Some(mesh) = render.meshes.get(mesh_renderer.mesh as usize) else {
                    continue;
                };
                let bounds: Aabb = mesh.bounds;
                instances.push(InstanceData::new(
                    global.0,
                    &bounds,
                    mesh_renderer.mesh,
                    mesh_renderer.material,
                ));
            }
        }
        self.render.instances = instances;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchy_accumulates() {
        let mut scene = Scene::new();
        let root = scene.spawn((
            Transform::from_translation(Vec3::new(1.0, 0.0, 0.0)),
            GlobalTransform::default(),
        ));
        let child = scene.spawn((
            Transform::from_translation(Vec3::new(0.0, 2.0, 0.0)),
            GlobalTransform::default(),
        ));
        scene.set_parent(child, root);
        scene.extract(1.0);
        let global = scene.world.get::<GlobalTransform>(child).unwrap();
        assert!((global.translation() - Vec3::new(1.0, 2.0, 0.0)).length() < 1e-4);
    }

    #[test]
    fn extracts_instances_and_camera() {
        let mut scene = Scene::new();
        let mesh = scene.add_mesh(renderer::cube(1.0));
        let material = scene.add_material(MaterialData::default());
        scene.spawn_mesh(mesh, material, Transform::default());
        scene.spawn_camera(
            Transform::from_translation(Vec3::new(0.0, 0.0, 5.0)),
            1.0,
            0.1,
        );
        scene.extract(16.0 / 9.0);
        assert_eq!(scene.render().instances.len(), 1);
        assert!((scene.render().camera.position - Vec3::new(0.0, 0.0, 5.0)).length() < 1e-4);
    }
}
