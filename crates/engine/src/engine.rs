use std::time::Instant;

use foundation::{Aabb, Mat4, Transform, Vec3};
use graphics::{Gpu, GpuConfig, Surface};
use platform::{Flow, Input, WindowRef};
use renderer::{DEFAULT_TEXTURE_LAYERS, MeshData, RenderOutcome, Renderer, RendererConfig};
use scene::Scene;

use crate::camera::FlyCamera;
use crate::config::EngineConfig;

#[derive(Default)]
struct Stats {
    fps: f32,
    frame_ms: f32,
    instances: u32,
}

struct ModelRuntime {
    root: scene::Entity,
    nodes: Vec<Option<scene::Entity>>,
    base_transforms: Vec<Transform>,
    animations: Vec<assets::AnimationClip>,
    current: Option<usize>,
    time: f32,
    speed: f32,
    looping: bool,
}

pub struct Engine {
    gpu: Gpu,
    surface: Surface,
    renderer: Renderer,
    scene: Scene,
    window: WindowRef,
    camera: FlyCamera,
    camera_entity: scene::Entity,
    config: EngineConfig,
    stats: Stats,
    last_frame: Instant,
    title_timer: f32,
    models: Vec<ModelRuntime>,
    model_bounds: Vec<(scene::Entity, Aabb)>,
}

impl Engine {
    pub fn new(config: &EngineConfig, window: WindowRef) -> Result<Self, graphics::GpuError> {
        let (width, height) = window.size();
        let gpu_config = GpuConfig {
            features: RendererConfig::required_features(),
            label: "flux",
            ..GpuConfig::default()
        };
        let (gpu, surface) =
            Gpu::new_for_window(&gpu_config, window.clone(), width.max(1), height.max(1))?;
        let renderer = Renderer::new(&gpu, config.renderer.clone(), surface.format());

        let mut scene = Scene::new();
        let camera = FlyCamera::default();
        let camera_entity = scene.spawn_camera(camera.transform(), 60f32.to_radians(), 0.1);
        scene.set_name(camera_entity, "main camera");
        scene.set_active_camera(camera_entity);

        Ok(Self {
            gpu,
            surface,
            renderer,
            scene,
            window,
            camera,
            camera_entity,
            config: config.clone(),
            stats: Stats::default(),
            last_frame: Instant::now(),
            title_timer: 0.0,
            models: Vec::new(),
            model_bounds: Vec::new(),
        })
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    pub fn set_camera_pose(&mut self, position: Vec3, yaw: f32, pitch: f32) {
        self.camera.position = position;
        self.camera.yaw = yaw;
        self.camera.pitch = pitch;
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface.resize(&self.gpu, width, height);
    }

    pub fn update_camera(&mut self, input: &Input, dt: f32) {
        self.camera.update(input, dt, &self.window);
        self.scene
            .set_camera_transform(self.camera_entity, self.camera.transform());
    }

    pub fn render_frame(&mut self) -> Flow {
        let (width, height) = self.surface.size();
        let aspect = width as f32 / height.max(1) as f32;
        let render_scene = self.scene.extract(aspect);

        match self.renderer.render(render_scene, &self.surface) {
            RenderOutcome::Presented | RenderOutcome::Skipped => {}
            RenderOutcome::Outdated => self.surface.reconfigure(&self.gpu),
            RenderOutcome::Lost => {
                match self.gpu.create_surface(self.window.clone(), width, height) {
                    Ok(surface) => self.surface = surface,
                    Err(error) => {
                        log::error!("lost surface could not be recreated: {error}");
                        return Flow::Exit;
                    }
                }
            }
        }

        self.update_stats();
        Flow::Continue
    }

    pub fn save_screenshot(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let (width, height) = self.surface.size();
        let aspect = width as f32 / height.max(1) as f32;
        let stats = {
            let render_scene = self.scene.extract(aspect);
            self.renderer.render_offscreen(render_scene, width, height)
        };
        let pixels = self
            .renderer
            .read_offscreen()
            .ok_or("failed to read back offscreen frame")?;
        image::RgbaImage::from_raw(width, height, pixels)
            .ok_or("unexpected pixel dimensions")?
            .save(path)?;
        log::info!("screenshot saved: {path} ({} instances)", stats.instances);
        Ok(())
    }

    pub fn spawn_model(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<scene::Entity, assets::AssetError> {
        let path = path.as_ref();
        let model = assets::load_gltf(path)?;
        let bounds = model_bounds(&model);
        let texture_offset = self.scene.render().textures.len() as u32;
        let material_offset = self.scene.render().materials.len() as u32;
        let mesh_count = model.meshes.len();
        let material_count = model.materials.len();
        let texture_count = model.textures.len();
        let primitive_count: usize = model.nodes.iter().map(|node| node.primitives.len()).sum();
        let animation_names: Vec<String> = model
            .animations
            .iter()
            .map(|clip| clip.name.clone())
            .collect();

        for texture in model.textures {
            self.scene.add_texture(texture);
        }
        for material in model.materials {
            let mut material = material;
            for texture in &mut material.textures {
                if *texture >= DEFAULT_TEXTURE_LAYERS {
                    *texture += texture_offset;
                }
            }
            self.scene.add_material(material);
        }

        let mesh_base: Vec<u32> = model
            .meshes
            .iter()
            .map(|mesh| {
                self.scene.add_mesh(MeshData {
                    vertices: mesh.vertices.clone(),
                    indices: mesh.indices.clone(),
                    bounds: mesh.bounds,
                })
            })
            .collect();

        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("model");
        let root = self.scene.spawn((
            Transform::default(),
            scene::GlobalTransform::default(),
            scene::Name(name.to_string()),
        ));

        let node_count = model.nodes.len();
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); node_count];
        for (index, node) in model.nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                children[parent].push(index);
            }
        }
        let mut reachable = vec![false; node_count];
        let mut pending = model.roots.clone();
        while let Some(index) = pending.pop() {
            if index >= node_count || reachable[index] {
                continue;
            }
            reachable[index] = true;
            pending.extend(children[index].iter().copied());
        }

        let mut node_entities: Vec<Option<scene::Entity>> = vec![None; node_count];
        let mut base_transforms = vec![Transform::default(); node_count];
        for (index, node) in model.nodes.iter().enumerate() {
            if !reachable[index] {
                continue;
            }
            base_transforms[index] = node.transform;
            let entity = self.scene.spawn((
                node.transform,
                scene::GlobalTransform(node.transform.matrix()),
                scene::Name(node.name.clone()),
            ));
            node_entities[index] = Some(entity);
        }
        for (index, node) in model.nodes.iter().enumerate() {
            let Some(Some(entity)) = node_entities.get(index).copied() else {
                continue;
            };
            let parent = node
                .parent
                .and_then(|parent| node_entities.get(parent).copied().flatten())
                .unwrap_or(root);
            self.scene.set_parent(entity, parent);
        }
        for (index, node) in model.nodes.iter().enumerate() {
            let Some(Some(node_entity)) = node_entities.get(index).copied() else {
                continue;
            };
            for primitive in &node.primitives {
                let mesh = mesh_base.get(primitive.mesh as usize).copied().unwrap_or(0);
                let entity = self.scene.spawn_mesh(
                    mesh,
                    material_offset + primitive.material,
                    Transform::default(),
                );
                self.scene.set_parent(entity, node_entity);
                let Some(skin) = primitive
                    .skin
                    .and_then(|skin| model.skins.get(skin as usize))
                else {
                    continue;
                };
                let joints: Vec<scene::Entity> = skin
                    .joints
                    .iter()
                    .filter_map(|joint| node_entities.get(*joint).copied().flatten())
                    .collect();
                if joints.len() != skin.joints.len() {
                    continue;
                }
                let _ = self.scene.world_mut().insert(
                    entity,
                    scene::Skin {
                        joints,
                        inverse_bind_matrices: skin.inverse_bind_matrices.clone(),
                    },
                );
            }
        }

        let animations = model.animations;
        if !animations.is_empty() {
            self.models.push(ModelRuntime {
                root,
                nodes: node_entities,
                base_transforms,
                animations,
                current: None,
                time: 0.0,
                speed: 1.0,
                looping: true,
            });
        }

        self.model_bounds
            .retain(|(entity, _)| self.scene.world().get::<Transform>(*entity).is_some());
        self.model_bounds.push((root, bounds));

        log::info!(
            "loaded model {}: {} nodes, {} primitives, {} meshes, {} materials, {} animations",
            path.display(),
            node_count,
            primitive_count,
            mesh_count,
            material_count,
            animation_names.len()
        );
        for name in &animation_names {
            log::info!("animation: {name}");
        }
        let _ = (texture_count, animation_names);
        Ok(root)
    }

    pub fn frame_model(&mut self, model: scene::Entity) -> bool {
        let Some((_, bounds)) = self
            .model_bounds
            .iter()
            .find(|(entity, _)| *entity == model)
            .copied()
        else {
            return false;
        };
        if !bounds.is_valid() {
            return false;
        }
        let center = bounds.center();
        let radius = bounds.extents().length().max(0.5);
        let direction = Vec3::new(0.65, 0.45, 0.85).normalize();
        let distance = radius / (30f32.to_radians()).sin() * 1.2;
        let pitch = (-direction.y).asin();
        let yaw = direction.x.atan2(direction.z);
        self.set_camera_pose(center + direction * distance, yaw, pitch);
        self.scene
            .set_camera_transform(self.camera_entity, self.camera.transform());
        true
    }

    pub fn animation_count(&self, model: scene::Entity) -> usize {
        self.model_runtime(model)
            .map(|runtime| runtime.animations.len())
            .unwrap_or(0)
    }

    pub fn animation_name(&self, model: scene::Entity, index: usize) -> Option<&str> {
        self.model_runtime(model)
            .and_then(|runtime| runtime.animations.get(index))
            .map(|clip| clip.name.as_str())
    }

    pub fn play_animation(&mut self, model: scene::Entity, index: usize) -> bool {
        let Some(runtime) = self.model_runtime_mut(model) else {
            return false;
        };
        if index >= runtime.animations.len() {
            return false;
        }
        runtime.current = Some(index);
        runtime.time = 0.0;
        true
    }

    pub fn set_animation_looping(&mut self, model: scene::Entity, looping: bool) {
        if let Some(runtime) = self.model_runtime_mut(model) {
            runtime.looping = looping;
        }
    }

    pub fn set_animation_speed(&mut self, model: scene::Entity, speed: f32) {
        if let Some(runtime) = self.model_runtime_mut(model) {
            runtime.speed = speed;
        }
    }

    pub fn advance_animations(&mut self, dt: f32) {
        let mut updates: Vec<(scene::Entity, Transform)> = Vec::new();
        for runtime in &mut self.models {
            let Some(index) = runtime.current else {
                continue;
            };
            let Some(clip) = runtime.animations.get(index) else {
                continue;
            };
            runtime.time += dt * runtime.speed;
            if clip.duration > 0.0 {
                runtime.time = if runtime.looping {
                    runtime.time.rem_euclid(clip.duration)
                } else {
                    runtime.time.min(clip.duration)
                };
            }
            for pose in clip.sample(runtime.time) {
                let Some(Some(entity)) = runtime.nodes.get(pose.node).copied() else {
                    continue;
                };
                let Some(base) = runtime.base_transforms.get(pose.node).copied() else {
                    continue;
                };
                let mut transform = base;
                if let Some(translation) = pose.translation {
                    transform.translation = translation;
                }
                if let Some(rotation) = pose.rotation {
                    transform.rotation = rotation;
                }
                if let Some(scale) = pose.scale {
                    transform.scale = scale;
                }
                updates.push((entity, transform));
            }
        }
        for (entity, transform) in updates {
            if let Some(mut slot) = self.scene.world_mut().get_mut::<Transform>(entity) {
                *slot = transform;
            }
        }
    }

    fn model_runtime(&self, model: scene::Entity) -> Option<&ModelRuntime> {
        self.models.iter().find(|runtime| runtime.root == model)
    }

    fn model_runtime_mut(&mut self, model: scene::Entity) -> Option<&mut ModelRuntime> {
        self.models.iter_mut().find(|runtime| runtime.root == model)
    }

    fn update_stats(&mut self) {
        let now = Instant::now();
        let seconds = (now - self.last_frame).as_secs_f32().max(1e-5);
        self.last_frame = now;
        self.stats.frame_ms = seconds * 1000.0;
        let fps = 1.0 / seconds;
        self.stats.fps = if self.stats.fps == 0.0 {
            fps
        } else {
            self.stats.fps + (fps - self.stats.fps) * 0.05
        };
        self.stats.instances = self.scene.render().instances.len() as u32;

        if self.config.show_fps {
            self.title_timer += seconds;
            if self.title_timer > 0.5 {
                self.title_timer = 0.0;
                self.window.set_title(&format!(
                    "{} | {:.0} FPS | {:.2} ms | {} instances",
                    self.config.window.title,
                    self.stats.fps,
                    self.stats.frame_ms,
                    self.stats.instances
                ));
            }
        }
    }
}

fn model_bounds(model: &assets::LoadedModel) -> Aabb {
    let mut worlds = vec![None; model.nodes.len()];
    for index in 0..model.nodes.len() {
        node_world(model, index, &mut worlds);
    }
    let mut bounds = Aabb::invalid();
    for (index, node) in model.nodes.iter().enumerate() {
        let Some(world) = worlds[index] else {
            continue;
        };
        for primitive in &node.primitives {
            let Some(mesh) = model.meshes.get(primitive.mesh as usize) else {
                continue;
            };
            bounds = bounds.union(&mesh.bounds.transform(&world));
        }
    }
    bounds
}

fn node_world(
    model: &assets::LoadedModel,
    index: usize,
    worlds: &mut [Option<Mat4>],
) -> Option<Mat4> {
    if let Some(world) = worlds.get(index).copied().flatten() {
        return Some(world);
    }
    let node = model.nodes.get(index)?;
    let local = node.transform.matrix();
    let world = match node.parent {
        Some(parent) => node_world(model, parent, worlds)? * local,
        None => local,
    };
    worlds[index] = Some(world);
    Some(world)
}
