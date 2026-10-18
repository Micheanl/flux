use std::time::Instant;

use foundation::{Transform, Vec3};
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
        let texture_offset = self.scene.render().textures.len() as u32;
        let material_offset = self.scene.render().materials.len() as u32;
        let mesh_count = model.meshes.len();
        let material_count = model.materials.len();
        let texture_count = model.textures.len();
        let instance_count = model.instances.len();

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

        for instance in &model.instances {
            let mesh = mesh_base.get(instance.mesh as usize).copied().unwrap_or(0);
            let entity = self.scene.spawn_mesh(
                mesh,
                material_offset + instance.material,
                instance.transform,
            );
            self.scene.set_parent(entity, root);
            self.scene.set_name(entity, &instance.name);
        }

        log::info!(
            "loaded model {}: {} meshes, {} materials, {} textures, {} instances",
            path.display(),
            mesh_count,
            material_count,
            texture_count,
            instance_count
        );
        Ok(root)
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
