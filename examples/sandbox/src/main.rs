use engine::{
    App, DirectionalLight, Engine, EngineConfig, Entity, Flow, FrameTime, Input, KeyCode,
    MaterialData, PointLight, Quat, RenderSettings, Transform, Vec3, Vec4, checker_texture, cube,
    plane, torus, uv_sphere,
};

struct Sandbox {
    spinners: Vec<(Entity, f32)>,
    sun: Entity,
    time: f32,
    capture: Option<String>,
    frame_limit: u64,
    frame: u64,
    model_path: Option<std::path::PathBuf>,
    model_root: Option<Entity>,
    animation_count: usize,
    animation_index: usize,
}

impl App for Sandbox {
    fn setup(engine: &mut Engine) -> Self {
        let capture = std::env::var("FLUX_CAPTURE").ok();
        let frame_limit = std::env::var("FLUX_FRAMES")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(u64::MAX);
        let model_path = std::env::var("FLUX_MODEL")
            .ok()
            .map(std::path::PathBuf::from);
        let mut spinners = Vec::new();

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

        let mut model_root = None;
        let mut animation_count = 0;
        let animation_index = 0;
        if let Some(path) = &model_path {
            match engine.spawn_model(path) {
                Ok(root) => {
                    model_root = Some(root);
                    animation_count = engine.animation_count(root);
                    if animation_count > 0 {
                        engine.play_animation(root, 0);
                    } else {
                        spinners.push((root, 0.35));
                    }
                    engine.set_camera_pose(Vec3::new(0.0, 1.6, 5.0), 0.0, -0.12);
                    log::info!("model mode: {}", path.display());
                }
                Err(error) => log::error!("model loading failed: {error}"),
            }
        }
        if model_root.is_none() {
            setup_showcase(engine, &mut spinners);
            engine.set_camera_pose(Vec3::new(0.0, 6.0, 26.0), 0.0, -0.18);
        }

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

        Self {
            spinners,
            sun,
            time: 0.0,
            capture,
            frame_limit,
            frame: 0,
            model_path,
            model_root,
            animation_count,
            animation_index,
        }
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

        self.frame += 1;
        if self.frame >= self.frame_limit {
            if let Some(path) = self.capture.clone()
                && let Err(error) = engine.save_screenshot(&path)
            {
                log::error!("screenshot failed: {error}");
            }
            return Flow::Exit;
        }

        self.time += time.delta;
        self.animate(engine);
        Flow::Continue
    }
}

impl Sandbox {
    fn reload_model(&mut self, engine: &mut Engine) {
        let Some(path) = self.model_path.clone() else {
            return;
        };
        if let Some(root) = self.model_root.take() {
            engine.scene_mut().despawn_recursive(root);
        }
        match engine.spawn_model(&path) {
            Ok(root) => {
                self.model_root = Some(root);
                self.spinners.retain(|(entity, _)| *entity != root);
                self.animation_count = engine.animation_count(root);
                self.animation_index = 0;
                if self.animation_count > 0 {
                    engine.play_animation(root, 0);
                } else {
                    self.spinners.push((root, 0.35));
                }
                log::info!("model reloaded: {}", path.display());
            }
            Err(error) => log::error!("model reload failed: {error}"),
        }
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
    engine::run::<Sandbox>(EngineConfig::default())?;
    Ok(())
}
