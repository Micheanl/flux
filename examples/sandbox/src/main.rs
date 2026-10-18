use std::path::{Path, PathBuf};
use std::time::Instant;

use cloth::Cloth;
use engine::{
    Aabb, App, DirectionalLight, Engine, EngineConfig, Entity, Flow, FrameTime, Input, KeyCode,
    MaterialData, MeshData, MouseButton, PointLight, Quat, RenderSettings, Transform, Vec2, Vec3,
    Vec4, Vertex, checker_texture, cube, plane, torus, uv_sphere,
};
use physics::CharacterController;

const MODE_KEYS: [(KeyCode, Mode); 5] = [
    (KeyCode::Digit1, Mode::Physics),
    (KeyCode::Digit2, Mode::Models),
    (KeyCode::Digit3, Mode::Cloth),
    (KeyCode::Digit4, Mode::Stress),
    (KeyCode::Digit5, Mode::Character),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Physics,
    Models,
    Cloth,
    Stress,
    Character,
}

impl Mode {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "physics" => Some(Self::Physics),
            "models" => Some(Self::Models),
            "cloth" => Some(Self::Cloth),
            "stress" => Some(Self::Stress),
            "character" => Some(Self::Character),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Physics => "physics",
            Self::Models => "models",
            Self::Cloth => "cloth",
            Self::Stress => "stress",
            Self::Character => "character",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Physics => "WASD move, right mouse look, space jump, tab fly",
            Self::Models => "left/right switch model, N animation, R reload",
            Self::Cloth => "wind simulation",
            Self::Stress => "instanced scene, frame rate in log",
            Self::Character => {
                "WASD move, shift run, right mouse orbit, space jump, V camera, T toon, N motion"
            }
        }
    }
}

struct ClothDemo {
    mesh: u32,
    simulation: Cloth,
    obstacle: Vec3,
}

struct CharacterDemo {
    root: Entity,
    player: CharacterController,
    yaw: f32,
    pitch: f32,
    facing: f32,
    third_person: bool,
    camera_distance: f32,
    camera_position: Vec3,
    motions: Vec<PathBuf>,
    motion_index: usize,
    idle_motion: Option<usize>,
    walk_motion: Option<usize>,
    run_motion: Option<usize>,
    free_motion: bool,
}

struct Sandbox {
    mode: Mode,
    time: f32,
    frame: u64,
    capture: Option<String>,
    capture_each: Option<PathBuf>,
    captured: bool,
    frame_limit: u64,
    sun: Entity,
    player: CharacterController,
    player_mode: bool,
    player_yaw: f32,
    player_pitch: f32,
    auto_walk: bool,
    models: Vec<PathBuf>,
    model_index: usize,
    model_root: Option<Entity>,
    animation_count: usize,
    animation_index: usize,
    auto_cycle: bool,
    cloth: Option<ClothDemo>,
    stress_count: u32,
    stress_seconds: f64,
    stress_start: Option<Instant>,
    character: Option<CharacterDemo>,
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
        let stress_count = std::env::var("FLUX_STRESS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(65_536);
        let models = collect_models();
        let mode = std::env::var("FLUX_MODE")
            .ok()
            .and_then(|name| Mode::from_name(&name))
            .unwrap_or(if std::env::var("FLUX_STRESS").is_ok() {
                Mode::Stress
            } else {
                Mode::Physics
            });

        let mut sandbox = Self {
            mode,
            time: 0.0,
            frame: 0,
            capture,
            capture_each,
            captured: false,
            frame_limit,
            sun: Entity::DANGLING,
            player: CharacterController::new(Vec3::new(0.0, 0.05, 18.0), 0.35, 1.8),
            player_mode: true,
            player_yaw: 0.0,
            player_pitch: -0.06,
            auto_walk,
            models,
            model_index: 0,
            model_root: None,
            animation_count: 0,
            animation_index: 0,
            auto_cycle,
            cloth: None,
            stress_count,
            stress_seconds: 0.0,
            stress_start: None,
            character: None,
        };
        sandbox.switch_mode(engine, mode);
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
        for (key, mode) in MODE_KEYS {
            if input.key_pressed(key) && self.mode != mode {
                self.switch_mode(engine, mode);
            }
        }
        if input.key_pressed(KeyCode::KeyR) {
            self.reload(engine);
        }

        self.frame += 1;
        self.time += time.delta;
        match self.mode {
            Mode::Physics => self.update_physics(engine, input, time.delta),
            Mode::Models => self.update_models(engine, input),
            Mode::Cloth => self.update_cloth(engine, time.delta),
            Mode::Stress => self.track_stress(),
            Mode::Character => self.update_character(engine, input, time.delta),
        }
        self.capture_once(engine);

        if self.frame >= self.frame_limit {
            if let Some(count) = self.stress_log()
                && let Some(start) = self.stress_start
            {
                log::info!(
                    "stress {count}: {} frames in {:.2}s ({:.1} fps)",
                    self.frame,
                    start.elapsed().as_secs_f64(),
                    self.frame as f64 / start.elapsed().as_secs_f64().max(1e-6)
                );
            }
            if let Some(path) = self.capture.clone()
                && let Err(error) = engine.save_screenshot(&path)
            {
                log::error!("screenshot failed: {error}");
            }
            return Flow::Exit;
        }
        Flow::Continue
    }
}

impl Sandbox {
    fn switch_mode(&mut self, engine: &mut Engine, mode: Mode) {
        engine.reset_scene();
        self.mode = mode;
        self.cloth = None;
        self.character = None;
        self.model_root = None;
        self.animation_count = 0;
        self.animation_index = 0;
        self.captured = false;
        self.player = CharacterController::new(Vec3::new(0.0, 0.05, 18.0), 0.35, 1.8);
        self.player_yaw = 0.0;
        self.player_pitch = -0.06;
        self.player_mode = mode == Mode::Physics;
        self.stress_start = (mode == Mode::Stress).then(Instant::now);
        self.stress_seconds = 0.0;

        self.sun = setup_world(engine);

        match mode {
            Mode::Physics => setup_arena(engine),
            Mode::Models => {
                self.load_model(engine, 0);
            }
            Mode::Cloth => self.cloth = Some(setup_cloth_scene(engine)),
            Mode::Stress => setup_stress(engine, self.stress_count),
            Mode::Character => self.enter_character(engine),
        }

        engine.set_title(&format!(
            "Flux | {} | 1-5 modes | {} | F12 screenshot",
            mode.label(),
            mode.hint()
        ));
        log::info!("mode: {} ({})", mode.label(), mode.hint());
    }

    fn reload(&mut self, engine: &mut Engine) {
        match self.mode {
            Mode::Models => self.load_model(engine, self.model_index),
            Mode::Physics => self.switch_mode(engine, Mode::Physics),
            Mode::Cloth => self.switch_mode(engine, Mode::Cloth),
            Mode::Stress => self.switch_mode(engine, Mode::Stress),
            Mode::Character => self.switch_mode(engine, Mode::Character),
        }
    }

    fn enter_character(&mut self, engine: &mut Engine) {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let Some(model) = character_model_path(manifest) else {
            log::error!("character model not found");
            return;
        };
        let root = match engine.spawn_model(&model) {
            Ok(root) => root,
            Err(error) => {
                log::error!("character model loading failed: {error}");
                return;
            }
        };
        let motions = motion_paths(manifest);
        for motion in &motions {
            match engine.add_animation(root, motion) {
                Ok(index) => log::info!(
                    "motion {index}: {}",
                    engine.animation_name(root, index).unwrap_or("?")
                ),
                Err(error) => log::warn!("cannot load motion {}: {error}", motion.display()),
            }
        }
        let motion_count = engine.animation_count(root);
        if motion_count > 0 {
            engine.play_animation(root, 0);
        }
        let find_motion = |needle: &str| {
            motions.iter().position(|path| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|name| name.to_lowercase().contains(needle))
            })
        };
        let idle_motion = find_motion("idle").or(if motions.is_empty() { None } else { Some(0) });
        let walk_motion = find_motion("walk");
        let run_motion = find_motion("run");
        let toon = std::env::var("FLUX_TOON")
            .map(|value| value != "0")
            .unwrap_or(true);
        {
            let settings = &mut engine.scene_mut().render_mut().settings;
            settings.toon = toon;
            if toon {
                settings.bloom = 0.02;
                settings.exposure = 0.9;
            }
        }
        engine.frame_model(root);
        let player = CharacterController::new(Vec3::new(0.0, 0.05, 0.0), 0.35, 1.8);
        self.character = Some(CharacterDemo {
            root,
            player,
            yaw: 0.0,
            pitch: 0.12,
            facing: std::f32::consts::PI,
            third_person: true,
            camera_distance: 3.4,
            camera_position: Vec3::new(0.0, 1.9, 3.4),
            motions,
            motion_index: idle_motion.unwrap_or(0),
            idle_motion,
            walk_motion,
            run_motion,
            free_motion: false,
        });
        log::info!("character: {} with {motion_count} motions", model.display());
    }

    fn update_character(&mut self, engine: &mut Engine, input: &Input, dt: f32) {
        let Some(state) = &mut self.character else {
            return;
        };
        if input.mouse_held(MouseButton::Right) {
            let motion = input.raw_motion();
            state.yaw -= motion.x * 0.0022;
            state.pitch = (state.pitch - motion.y * 0.0022).clamp(-0.85, 0.85);
        }
        let scroll = input.scroll();
        if scroll.abs() > 0.0 {
            state.camera_distance = (state.camera_distance - scroll * 0.4).clamp(1.6, 7.0);
        }
        if input.key_pressed(KeyCode::KeyV) {
            state.third_person = !state.third_person;
            log::info!(
                "camera: {}",
                if state.third_person {
                    "third person"
                } else {
                    "first person"
                }
            );
        }
        if input.key_pressed(KeyCode::KeyT) {
            let settings = &mut engine.scene_mut().render_mut().settings;
            settings.toon = !settings.toon;
            log::info!("toon shading: {}", settings.toon);
        }
        if input.key_pressed(KeyCode::KeyN) && !state.motions.is_empty() {
            state.motion_index = (state.motion_index + 1) % state.motions.len();
            state.free_motion = true;
            engine.play_animation(state.root, state.motion_index);
            log::info!(
                "motion: {}",
                engine
                    .animation_name(state.root, state.motion_index)
                    .unwrap_or("?")
            );
        }
        let yaw = state.yaw;
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
        if self.auto_walk && wish.length_squared() < 1e-4 {
            wish = forward;
        }
        let moving = wish.length_squared() > 1e-4;
        let speed = if input.key_held(KeyCode::ShiftLeft) {
            4.2
        } else {
            1.8
        };
        let jump = if input.key_pressed(KeyCode::Space) {
            7.5
        } else {
            0.0
        };
        let excluded = engine.scene().subtree(state.root);
        let colliders = engine.scene().collision_boxes_except(&excluded);
        state.player.update(&colliders, 0.0, wish, speed, jump, dt);
        if moving {
            let target = yaw + std::f32::consts::PI;
            state.facing = rotate_towards(state.facing, target, 10.0 * dt);
        }
        let position = state.player.position;
        let facing = state.facing;
        if let Some(mut transform) = engine
            .scene_mut()
            .world_mut()
            .get_mut::<Transform>(state.root)
        {
            transform.translation = position;
            transform.rotation = Quat::from_rotation_y(facing);
        }
        let running = input.key_held(KeyCode::ShiftLeft) || input.key_held(KeyCode::ShiftRight);
        if moving {
            state.free_motion = false;
        }
        if !state.free_motion {
            let desired = if moving {
                if running {
                    state.run_motion.or(state.walk_motion)
                } else {
                    state.walk_motion
                }
            } else {
                state.idle_motion
            };
            if let Some(desired) = desired
                && desired != state.motion_index
            {
                engine.play_animation(state.root, desired);
                state.motion_index = desired;
            }
        }
        let target = position + Vec3::Y * 1.35;
        if state.third_person {
            let pitch = state.pitch;
            let forward = Vec3::new(
                -yaw.sin() * pitch.cos(),
                pitch.sin(),
                -yaw.cos() * pitch.cos(),
            );
            let mut desired = target - forward * state.camera_distance;
            desired.y = desired.y.max(0.35);
            let direction = (desired - target).normalize_or_zero();
            let mut distance = state.camera_distance;
            let colliders = engine.scene().collision_boxes_except(&excluded);
            for obb in &colliders {
                if let Some(hit) = obb.ray_intersection(target, direction)
                    && hit < distance
                {
                    distance = (hit - 0.2).max(0.5);
                }
            }
            let desired = target + direction * distance;
            let smoothing = 1.0 - (-14.0 * dt).exp();
            state.camera_position = state.camera_position.lerp(desired, smoothing);
            let (camera_yaw, camera_pitch) = looking_at(state.camera_position, target);
            engine.set_camera_pose(state.camera_position, camera_yaw, camera_pitch);
        } else {
            engine.set_camera_pose(state.player.eye(), state.yaw, state.pitch);
        }
    }

    fn update_physics(&mut self, engine: &mut Engine, input: &Input, dt: f32) {
        if input.key_pressed(KeyCode::Tab) {
            self.player_mode = !self.player_mode;
            log::info!(
                "camera mode: {}",
                if self.player_mode { "player" } else { "fly" }
            );
            engine.set_camera_pose(self.player.eye(), self.player_yaw, self.player_pitch);
        }
        if !self.player_mode {
            return;
        }
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
        engine.set_camera_pose(self.player.eye(), self.player_yaw, self.player_pitch);
    }

    fn update_models(&mut self, engine: &mut Engine, input: &Input) {
        if input.key_pressed(KeyCode::ArrowRight) || input.key_pressed(KeyCode::PageDown) {
            self.step_model(engine, 1);
        }
        if input.key_pressed(KeyCode::ArrowLeft) || input.key_pressed(KeyCode::PageUp) {
            self.step_model(engine, -1);
        }
        if input.key_pressed(KeyCode::KeyN) {
            self.next_animation(engine);
        }
        if self.auto_cycle && !self.models.is_empty() && self.frame.is_multiple_of(40) {
            self.step_model(engine, 1);
        }
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
        demo.simulation.step(
            dt,
            Vec3::new(0.0, -9.8, 0.0) + wind,
            6,
            Some((demo.obstacle, 1.15)),
        );
        engine.update_mesh(demo.mesh, cloth_mesh(&demo.simulation));
    }

    fn track_stress(&mut self) {
        if self.stress_start.is_none() {
            return;
        }
        self.stress_seconds += 1.0 / 60.0;
        if self.frame.is_multiple_of(120) {
            let fps = 120.0 / self.stress_seconds.max(1e-6);
            log::info!("stress {}: {fps:.1} fps", self.stress_count);
            self.stress_seconds = 0.0;
        }
    }

    fn stress_log(&self) -> Option<u32> {
        (self.mode == Mode::Stress).then_some(self.stress_count)
    }

    fn capture_once(&mut self, engine: &mut Engine) {
        if self.captured {
            return;
        }
        let Some(directory) = &self.capture_each else {
            return;
        };
        let name = match self.mode {
            Mode::Models => self
                .models
                .get(self.model_index)
                .and_then(|path| path.file_stem())
                .and_then(|name| name.to_str())
                .unwrap_or("model"),
            other => other.label(),
        };
        let target = directory.join(format!("{name}.png"));
        self.captured = true;
        if let Some(target) = target.to_str()
            && let Err(error) = engine.save_screenshot(target)
        {
            log::error!("screenshot failed: {error}");
        }
    }

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
        }
        self.animation_count = 0;
        self.animation_index = 0;
    }

    fn step_model(&mut self, engine: &mut Engine, step: isize) {
        if self.models.is_empty() {
            return;
        }
        let count = self.models.len() as isize;
        self.load_model(
            engine,
            (self.model_index as isize + step).rem_euclid(count) as usize,
        );
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
}

fn character_model_path(manifest: &Path) -> Option<PathBuf> {
    let config = manifest.join("assets/character.txt");
    if let Ok(text) = std::fs::read_to_string(&config) {
        let relative = text.trim();
        if !relative.is_empty() {
            let path = manifest.join("assets").join(relative);
            if path.exists() {
                return Some(path);
            }
            log::warn!("configured character model missing: {}", path.display());
        }
    }
    let mut candidates = Vec::new();
    scan_model_files(&manifest.join("assets/models"), &mut candidates, 3);
    candidates
        .into_iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "pmx"))
        .max_by_key(|path| path.metadata().map(|meta| meta.len()).unwrap_or(0))
}

fn motion_paths(manifest: &Path) -> Vec<PathBuf> {
    let mut motions = Vec::new();
    match std::env::var("FLUX_MOTIONS_DIR") {
        Ok(directory) => scan_motion_files(&PathBuf::from(directory), &mut motions, 4),
        Err(_) => {
            scan_motion_files(&manifest.join("assets/animations"), &mut motions, 2);
            scan_motion_files(&manifest.join("assets/motions"), &mut motions, 4);
        }
    }
    motions.sort();
    motions
}

fn scan_model_files(directory: &Path, files: &mut Vec<PathBuf>, depth: usize) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_model_files(&path, files, depth - 1);
        } else if path.extension().is_some_and(|extension| {
            matches!(extension.to_str(), Some("pmx" | "pmd" | "glb" | "gltf"))
        }) {
            files.push(path);
        }
    }
}

fn scan_motion_files(directory: &Path, files: &mut Vec<PathBuf>, depth: usize) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_motion_files(&path, files, depth - 1);
        } else if path.extension().is_some_and(|extension| extension == "vmd") {
            files.push(path);
        }
    }
}

fn rotate_towards(current: f32, target: f32, max_step: f32) -> f32 {
    let difference = (target - current + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    current + difference.clamp(-max_step, max_step)
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
    } else {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/models");
        collect_directory(&directory, &mut models);
        models.sort();
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
            .is_some_and(|extension| matches!(extension.to_str(), Some("glb" | "gltf" | "pmx")))
        {
            models.push(path);
        }
    }
}

fn setup_world(engine: &mut Engine) -> Entity {
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

    let direction = Vec3::new(0.45, 0.8, 0.35).normalize();
    let sun = engine.scene_mut().spawn_sun(
        Transform::looking_at(direction * 30.0, Vec3::ZERO, Vec3::Y),
        DirectionalLight::default(),
    );

    let settings: &mut RenderSettings = &mut engine.scene_mut().render_mut().settings;
    settings.exposure = 0.85;
    settings.bloom = 0.06;
    settings.shadow_distance = 70.0;
    settings.ambient = 0.18;
    sun
}

fn setup_arena(engine: &mut Engine) {
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
        scene.spawn_mesh(
            sphere_mesh,
            if index % 2 == 0 { steel } else { gold },
            Transform::from_translation(Vec3::new(
                angle.cos() * radius,
                1.0 + (index % 4) as f32 * 0.4,
                angle.sin() * radius,
            )),
        );
    }

    for index in 0..6 {
        scene.spawn_mesh(
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
    let (yaw, pitch) = looking_at(position, Vec3::ZERO);
    engine.set_camera_pose(position, yaw, pitch);
    log::info!("stress scene: {count} instances, grid {side}x{side}, extent {extent:.0}");
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

fn looking_at(position: Vec3, target: Vec3) -> (f32, f32) {
    let direction = (target - position).normalize();
    let yaw = (-direction.x).atan2(-direction.z);
    let pitch = direction.y.asin();
    (yaw, pitch)
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
