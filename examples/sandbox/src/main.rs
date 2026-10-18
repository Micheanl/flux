use std::path::{Path, PathBuf};
use std::time::Instant;

use cloth::Cloth;
use engine::{
    Aabb, App, DirectionalLight, Engine, EngineConfig, Entity, Flow, FrameTime, Input, KeyCode,
    MaterialData, MeshData, MouseButton, PointLight, Quat, RenderSettings, Transform, Vec2, Vec3,
    Vec4, Vertex, checker_texture, cube, plane, torus, uv_sphere,
};
use physics::CharacterController;

struct Sandbox {
    spinners: Vec<(Entity, f32)>,
    sun: Entity,
    time: f32,
    capture: Option<String>,
    capture_each: Option<PathBuf>,
    captured: bool,
    frame_limit: u64,
    frame: u64,
    models: Vec<PathBuf>,
    model_index: usize,
    model_root: Option<Entity>,
    animation_count: usize,
    animation_index: usize,
    auto_cycle: bool,
    player: CharacterController,
    player_mode: bool,
    player_yaw: f32,
    player_pitch: f32,
    auto_walk: bool,
    stress: Option<u32>,
    stress_seconds: f64,
    bench_start: Option<Instant>,
    cloth: Option<ClothDemo>,
}

struct ClothDemo {
    mesh: u32,
    simulation: Cloth,
    obstacle: Vec3,
}

impl App for Sandbox {
    fn setup(engine: &mut Engine) -> Self {
        let capture = std::env::var("FLUX_CAPTURE").ok();
        let capture_each = std::env::var("FLUX_CAPTURE_EACH").ok().map(PathBuf::from);
        if let Some(directory) = &capture_each {
            std::fs::create_dir_all(directory).ok();
        }
        let frame_limit = std::env::var("FLUX_FRAMES")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(u64::MAX);
        let auto_cycle = std::env::var("FLUX_MODEL_AUTO").is_ok_and(|value| value == "1");
        let auto_walk = std::env::var("FLUX_PLAYER_AUTO").is_ok_and(|value| value == "1");
        let stress = std::env::var("FLUX_STRESS")
            .ok()
            .and_then(|value| value.parse().ok());
        let cloth_mode = std::env::var("FLUX_CLOTH").is_ok_and(|value| value == "1");
        let models = collect_models();

        let checker = engine.scene_mut().add_texture(checker_texture(
            512,
            64,
            [230, 230, 230, 255],
            [70, 70, 70, 255],
        ));
        let ground_mesh = engine.scene_mut().add_mesh(plane(120.0, 24));
        let ground_material = engine.scene_mut().add_material(
            MaterialData::pbr(Vec4::splat(1.0), 0.0, 0.9)
                .with_textures(checker, 1, 2, 3)
                .with_uv_scale([16.0, 16.0], [0.0, 0.0]),
        );
        engine.scene_mut().spawn_mesh(
            ground_mesh,
            ground_material,
            Transform::from_translation(Vec3::ZERO),
        );

        let sun = {
            let direction = Vec3::new(0.45, 0.8, 0.35).normalize();
            engine.scene_mut().spawn_sun(
                Transform::looking_at(direction * 30.0, Vec3::ZERO, Vec3::Y),
                DirectionalLight::default(),
            )
        };

        let settings: &mut RenderSettings = &mut engine.scene_mut().render_mut().settings;
        settings.exposure = 0.85;
        settings.bloom = 0.06;
        settings.shadow_distance = 70.0;
        settings.ambient = 0.18;

        let player_mode = stress.is_none() && !cloth_mode && (models.is_empty() || auto_walk);
        let mut sandbox = Self {
            spinners: Vec::new(),
            sun,
            time: 0.0,
            capture,
            capture_each,
            captured: false,
            frame_limit,
            frame: 0,
            models,
            model_index: 0,
            model_root: None,
            animation_count: 0,
            animation_index: 0,
            auto_cycle,
            player: CharacterController::new(Vec3::new(0.0, 0.05, 18.0), 0.35, 1.8),
            player_mode,
            player_yaw: 0.0,
            player_pitch: -0.06,
            auto_walk,
            stress,
            stress_seconds: 0.0,
            bench_start: stress.map(|_| Instant::now()),
            cloth: None,
        };

        if let Some(count) = stress {
            setup_stress(engine, count);
        } else if cloth_mode {
            sandbox.cloth = Some(setup_cloth_scene(engine));
        } else if sandbox.models.is_empty() {
            setup_showcase(engine, &mut sandbox.spinners);
            engine.set_camera_pose(
                sandbox.player.eye(),
                sandbox.player_yaw,
                sandbox.player_pitch,
            );
        } else {
            sandbox.load_model(engine, 0);
        }
        sandbox
    }

    fn update(&mut self, engine: &mut Engine, input: &Input, time: FrameTime) -> Flow {
        if input.key_pressed(KeyCode::Escape) {
            return Flow::Exit;
        }
        if input.key_pressed(KeyCode::F12)
            && let Err(error) = engine.save_screenshot("screenshot.png")
        {
            log::error!("screenshot failed: {error}");
        }
        if input.key_pressed(KeyCode::KeyR) {
            self.reload_model(engine);
        }
        if input.key_pressed(KeyCode::KeyN) {
            self.next_animation(engine);
        }
        if input.key_pressed(KeyCode::Tab) {
            self.player_mode = !self.player_mode;
            log::info!(
                "camera mode: {}",
                if self.player_mode { "player" } else { "fly" }
            );
            engine.set_camera_pose(self.player.eye(), self.player_yaw, self.player_pitch);
        }
        if input.key_pressed(KeyCode::ArrowRight) || input.key_pressed(KeyCode::PageDown) {
            self.step_model(engine, 1);
        }
        if input.key_pressed(KeyCode::ArrowLeft) || input.key_pressed(KeyCode::PageUp) {
            self.step_model(engine, -1);
        }

        self.frame += 1;
        self.track_stress(time.delta);
        if self.auto_cycle && !self.models.is_empty() && self.frame.is_multiple_of(40) {
            self.step_model(engine, 1);
        }
        self.capture_model_once(engine);
        if self.frame >= self.frame_limit {
            if let Some(count) = self.stress {
                let elapsed = self
                    .bench_start
                    .map_or(0.0, |start| start.elapsed().as_secs_f64());
                log::info!(
                    "stress {count}: {} frames in {elapsed:.2}s ({:.1} fps)",
                    self.frame,
                    self.frame as f64 / elapsed.max(1e-6)
                );
            }
            if let Some(path) = self.capture.clone()
                && let Err(error) = engine.save_screenshot(&path)
            {
                log::error!("screenshot failed: {error}");
            }
            return Flow::Exit;
        }

        self.time += time.delta;
        self.animate(engine);
        self.update_cloth(engine, time.delta);
        if self.player_mode {
            self.update_player(engine, input, time.delta);
        }
        Flow::Continue
    }
}

impl Sandbox {
    fn load_model(&mut self, engine: &mut Engine, index: usize) {
        let Some(path) = self.models.get(index).cloned() else {
            return;
        };
        self.clear_model(engine);
        self.model_index = index;
        match engine.spawn_model(&path) {
            Ok(root) => {
                self.model_root = Some(root);
                self.animation_count = engine.animation_count(root);
                self.animation_index = 0;
                if self.animation_count > 0 {
                    engine.play_animation(root, 0);
                } else {
                    self.spinners.push((root, 0.35));
                }
                engine.frame_model(root);
                log::info!(
                    "model {}/{}: {}",
                    index + 1,
                    self.models.len(),
                    path.display()
                );
            }
            Err(error) => log::error!("model loading failed: {error}"),
        }
        self.captured = false;
    }

    fn clear_model(&mut self, engine: &mut Engine) {
        if let Some(root) = self.model_root.take() {
            engine.scene_mut().despawn_recursive(root);
            self.spinners.retain(|(entity, _)| *entity != root);
        }
        self.animation_count = 0;
        self.animation_index = 0;
    }

    fn step_model(&mut self, engine: &mut Engine, step: isize) {
        if self.models.is_empty() {
            return;
        }
        let count = self.models.len() as isize;
        let index = (self.model_index as isize + step).rem_euclid(count) as usize;
        self.load_model(engine, index);
    }

    fn reload_model(&mut self, engine: &mut Engine) {
        if self.models.is_empty() {
            return;
        }
        self.load_model(engine, self.model_index);
    }

    fn next_animation(&mut self, engine: &mut Engine) {
        let (Some(root), true) = (self.model_root, self.animation_count > 1) else {
            return;
        };
        self.animation_index = (self.animation_index + 1) % self.animation_count;
        engine.play_animation(root, self.animation_index);
        log::info!(
            "animation: {}",
            engine
                .animation_name(root, self.animation_index)
                .unwrap_or("?")
        );
    }

    fn capture_model_once(&mut self, engine: &mut Engine) {
        if self.captured {
            return;
        }
        let (Some(directory), Some(path)) = (&self.capture_each, self.models.get(self.model_index))
        else {
            return;
        };
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("model");
        let target = directory.join(format!("{name}.png"));
        self.captured = true;
        if let Some(target) = target.to_str()
            && let Err(error) = engine.save_screenshot(target)
        {
            log::error!("screenshot failed: {error}");
        }
    }

    fn track_stress(&mut self, delta: f32) {
        let Some(count) = self.stress else {
            return;
        };
        self.stress_seconds += f64::from(delta);
        if self.frame.is_multiple_of(120) {
            let fps = 120.0 / self.stress_seconds.max(1e-6);
            log::info!("stress {count}: {fps:.1} fps");
            self.stress_seconds = 0.0;
        }
    }

    fn update_player(&mut self, engine: &mut Engine, input: &Input, dt: f32) {
        if input.mouse_held(MouseButton::Right) {
            let motion = input.raw_motion();
            self.player_yaw -= motion.x * 0.0022;
            self.player_pitch = (self.player_pitch - motion.y * 0.0022).clamp(-1.4, 1.4);
        }
        let yaw = self.player_yaw;
        let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
        let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
        let mut wish = Vec3::ZERO;
        if input.key_held(KeyCode::KeyW) {
            wish += forward;
        }
        if input.key_held(KeyCode::KeyS) {
            wish -= forward;
        }
        if input.key_held(KeyCode::KeyD) {
            wish += right;
        }
        if input.key_held(KeyCode::KeyA) {
            wish -= right;
        }
        if self.auto_walk {
            wish += forward;
        }
        let speed = if input.key_held(KeyCode::ShiftLeft) {
            9.0
        } else {
            5.0
        };
        let jump = if input.key_pressed(KeyCode::Space) {
            7.5
        } else {
            0.0
        };
        let colliders = engine.scene().collision_boxes();
        self.player.update(&colliders, 0.0, wish, speed, jump, dt);
        if self.auto_walk && self.frame.is_multiple_of(30) {
            log::info!(
                "player position {:?} grounded {}",
                self.player.position,
                self.player.grounded
            );
        }
        engine.set_camera_pose(self.player.eye(), self.player_yaw, self.player_pitch);
    }

    fn update_cloth(&mut self, engine: &mut Engine, dt: f32) {
        let Some(demo) = &mut self.cloth else {
            return;
        };
        let time = self.time;
        let wind = Vec3::new(
            (time * 0.7).sin() * 1.5,
            (time * 1.1).cos() * 1.8,
            9.0 + (time * 2.3).sin() * 6.0,
        );
        let acceleration = Vec3::new(0.0, -9.8, 0.0) + wind;
        demo.simulation
            .step(dt, acceleration, 6, Some((demo.obstacle, 1.15)));
        engine.update_mesh(demo.mesh, cloth_mesh(&demo.simulation));
    }

    fn animate(&mut self, engine: &mut Engine) {
        let elapsed = self.time;
        let sun_angle = elapsed * 0.08;
        let sun_direction =
            Vec3::new(sun_angle.cos() * 0.5, 0.8, sun_angle.sin() * 0.5).normalize();
        let model_root = self.model_root;
        let world = engine.scene_mut().world_mut();

        for (entity, speed) in &self.spinners {
            let Some(mut transform) = world.get_mut::<Transform>(*entity) else {
                continue;
            };
            transform.rotation = if Some(*entity) == model_root {
                Quat::from_rotation_y(elapsed * speed)
            } else {
                Quat::from_rotation_y(elapsed * speed)
                    * Quat::from_rotation_x(elapsed * speed * 0.5)
            };
        }

        if let Some(mut transform) = world.get_mut::<Transform>(self.sun) {
            *transform = Transform::looking_at(sun_direction * 30.0, Vec3::ZERO, Vec3::Y);
        }
    }
}

fn collect_models() -> Vec<PathBuf> {
    let mut models = Vec::new();
    if let Ok(directory) = std::env::var("FLUX_MODEL_DIR") {
        collect_directory(Path::new(&directory), &mut models);
        models.sort();
    } else if let Ok(path) = std::env::var("FLUX_MODEL") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            collect_directory(&path, &mut models);
            models.sort();
        } else {
            models.push(path);
        }
    }
    models
}

fn collect_directory(directory: &Path, models: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        log::error!("cannot read model directory {}", directory.display());
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "glb" || extension == "gltf")
        {
            models.push(path);
        }
    }
}

fn setup_stress(engine: &mut Engine, count: u32) {
    let cube_mesh = engine.scene_mut().add_mesh(cube(1.0));
    let material = engine.scene_mut().add_material(MaterialData::pbr(
        Vec4::new(0.8, 0.3, 0.22, 1.0),
        0.15,
        0.45,
    ));
    let side = (count as f64).sqrt().ceil() as u32;
    let spacing = 2.2;
    let half = (side as f32 - 1.0) * 0.5;
    let scene = engine.scene_mut();
    for index in 0..count {
        let x = (index % side) as f32;
        let z = (index / side) as f32;
        let height = 0.4 + (index.wrapping_mul(2_654_435_761) % 97) as f32 / 97.0 * 2.4;
        scene.spawn_mesh(
            cube_mesh,
            material,
            Transform {
                translation: Vec3::new((x - half) * spacing, height * 0.5, (z - half) * spacing),
                rotation: Quat::IDENTITY,
                scale: Vec3::new(0.75, height, 0.75),
            },
        );
    }
    let extent = side as f32 * spacing * 0.5;
    let position = Vec3::new(extent * 0.15, extent * 0.4, extent * 1.5);
    let (yaw, pitch) = looking_at(position, Vec3::new(0.0, 0.0, 0.0));
    engine.set_camera_pose(position, yaw, pitch);
    log::info!("stress scene: {count} instances, grid {side}x{side}, extent {extent:.0}");
}

fn looking_at(position: Vec3, target: Vec3) -> (f32, f32) {
    let direction = (target - position).normalize();
    let yaw = (-direction.x).atan2(-direction.z);
    let pitch = direction.y.asin();
    (yaw, pitch)
}

fn cloth_mesh(cloth: &Cloth) -> MeshData {
    let width = cloth.width();
    let height = cloth.height();
    let positions = cloth.positions();
    let mut vertices = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            let left = positions[y * width + x.saturating_sub(1)];
            let right = positions[y * width + (x + 1).min(width - 1)];
            let up = positions[y.saturating_sub(1) * width + x];
            let down = positions[(y + 1).min(height - 1) * width + x];
            let tangent = (right - left).normalize_or_zero();
            let normal = tangent
                .cross((up - down).normalize_or_zero())
                .normalize_or_zero();
            vertices.push(Vertex {
                position: positions[index].to_array(),
                normal: normal.to_array(),
                uv: [
                    x as f32 / (width - 1) as f32,
                    y as f32 / (height - 1) as f32,
                ],
                tangent: [tangent.x, tangent.y, tangent.z, 1.0],
                joints: [0; 4],
                weights: [0.0; 4],
            });
        }
    }
    let bounds = Aabb::from_points(positions.iter().copied());
    let bounds = Aabb::from_center_extents(bounds.center(), bounds.extents().max(Vec3::splat(0.1)));
    MeshData {
        vertices,
        indices: cloth.indices().to_vec(),
        bounds,
    }
}

fn setup_cloth_scene(engine: &mut Engine) -> ClothDemo {
    let steel = engine.scene_mut().add_material(MaterialData::pbr(
        Vec4::new(0.8, 0.82, 0.9, 1.0),
        1.0,
        0.18,
    ));
    let sphere_mesh = engine.scene_mut().add_mesh(uv_sphere(0.5, 48, 24));

    let simulation = Cloth::new(64, 40, Vec2::new(5.6, 3.6));
    let mesh = engine.scene_mut().add_mesh(cloth_mesh(&simulation));
    let texture = engine.scene_mut().add_texture(checker_texture(
        256,
        32,
        [220, 70, 60, 255],
        [180, 45, 40, 255],
    ));
    let material = engine.scene_mut().add_material(
        MaterialData::pbr(Vec4::splat(1.0), 0.0, 0.5)
            .with_textures(texture, 1, 2, 3)
            .with_uv_scale([3.5, 3.5], [0.0, 0.0])
            .with_double_sided(true),
    );
    let transform = Transform {
        translation: Vec3::new(-2.8, 3.4, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };
    engine.scene_mut().spawn_mesh(mesh, material, transform);
    let obstacle = Vec3::new(2.8, -2.1, 1.5);
    let obstacle_world = transform.matrix().transform_point3(obstacle);
    engine.scene_mut().spawn_mesh(
        sphere_mesh,
        steel,
        Transform {
            translation: obstacle_world,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(2.0),
        },
    );

    let camera = Vec3::new(0.0, 2.4, 6.6);
    let (yaw, pitch) = looking_at(camera, Vec3::new(0.0, 1.8, 0.4));
    engine.set_camera_pose(camera, yaw, pitch);

    ClothDemo {
        mesh,
        simulation,
        obstacle,
    }
}

fn setup_showcase(engine: &mut Engine, spinners: &mut Vec<(Entity, f32)>) {
    let (cube_mesh, sphere_mesh, torus_mesh) = {
        let scene = engine.scene_mut();
        (
            scene.add_mesh(cube(1.0)),
            scene.add_mesh(uv_sphere(0.5, 40, 20)),
            scene.add_mesh(torus(0.65, 0.22, 64, 24)),
        )
    };
    let tiles = engine.scene_mut().add_texture(checker_texture(
        256,
        32,
        [200, 120, 80, 255],
        [140, 70, 45, 255],
    ));
    let stone = engine.scene_mut().add_material(MaterialData::pbr(
        Vec4::new(0.85, 0.25, 0.18, 1.0),
        0.05,
        0.45,
    ));
    let tile = engine.scene_mut().add_material(
        MaterialData::pbr(Vec4::splat(1.0), 0.0, 0.55)
            .with_textures(tiles, 1, 2, 3)
            .with_uv_scale([2.0, 2.0], [0.0, 0.0]),
    );
    let steel = engine.scene_mut().add_material(MaterialData::pbr(
        Vec4::new(0.95, 0.95, 1.0, 1.0),
        1.0,
        0.15,
    ));
    let gold = engine.scene_mut().add_material(MaterialData::pbr(
        Vec4::new(1.0, 0.78, 0.35, 1.0),
        1.0,
        0.22,
    ));

    let scene = engine.scene_mut();
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut random = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as f32 / (1u32 << 31) as f32) * 2.0 - 1.0
    };

    for z in 0..14 {
        for x in 0..14 {
            let height = 0.4 + (x as f32 * 0.3 + z as f32 * 0.17).sin().abs() * 2.2;
            let material = match (x + z) % 3 {
                0 => stone,
                1 => tile,
                _ => steel,
            };
            scene.spawn_mesh(
                cube_mesh,
                material,
                Transform {
                    translation: Vec3::new(
                        x as f32 * 2.4 - 16.0,
                        height * 0.5,
                        z as f32 * 2.4 - 16.0,
                    ),
                    rotation: Quat::from_rotation_y(random() * 0.8),
                    scale: Vec3::new(0.8, height, 0.8),
                },
            );
        }
    }

    for index in 0..18 {
        let angle = index as f32 * std::f32::consts::TAU / 18.0;
        let radius = 9.0 + (index % 3) as f32 * 1.5;
        let entity = scene.spawn_mesh(
            sphere_mesh,
            if index % 2 == 0 { steel } else { gold },
            Transform::from_translation(Vec3::new(
                angle.cos() * radius,
                1.0 + (index % 4) as f32 * 0.4,
                angle.sin() * radius,
            )),
        );
        spinners.push((entity, 0.6 + index as f32 * 0.12));
    }

    for index in 0..6 {
        let entity = scene.spawn_mesh(
            torus_mesh,
            if index % 2 == 0 { gold } else { steel },
            Transform {
                translation: Vec3::new(
                    -4.0 + index as f32 * 1.8,
                    3.2 + (index as f32).sin() * 0.4,
                    -8.0,
                ),
                rotation: Quat::from_rotation_x(0.9),
                scale: Vec3::ONE,
            },
        );
        spinners.push((entity, 1.2));
    }

    for (index, color) in [
        Vec3::new(1.0, 0.25, 0.2),
        Vec3::new(0.2, 0.6, 1.0),
        Vec3::new(0.3, 1.0, 0.45),
    ]
    .into_iter()
    .enumerate()
    {
        let angle = index as f32 * std::f32::consts::TAU / 3.0;
        scene.spawn_point_light(
            Transform::from_translation(Vec3::new(angle.cos() * 6.0, 2.5, angle.sin() * 6.0)),
            PointLight::new(color, 18.0, 18.0),
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let mut config = EngineConfig::default();
    if let Some(count) = std::env::var("FLUX_STRESS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
    {
        config.renderer.max_instances = count.max(config.renderer.max_instances);
    }
    engine::run::<Sandbox>(config)?;
    Ok(())
}
