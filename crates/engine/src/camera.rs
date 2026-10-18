use foundation::{EulerRot, Quat, Transform, Vec3};
use platform::{Input, KeyCode, MouseButton, WindowRef};

pub struct FlyCamera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
    pub sensitivity: f32,
    looking: bool,
}

impl Default for FlyCamera {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 2.0, 6.0),
            yaw: 0.0,
            pitch: -0.15,
            speed: 6.0,
            sensitivity: 0.0022,
            looking: false,
        }
    }
}

impl FlyCamera {
    pub fn new(position: Vec3, yaw: f32, pitch: f32) -> Self {
        Self {
            position,
            yaw,
            pitch,
            ..Self::default()
        }
    }

    pub fn transform(&self) -> Transform {
        Transform {
            translation: self.position,
            rotation: Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0),
            scale: Vec3::ONE,
        }
    }

    pub fn update(&mut self, input: &Input, dt: f32, window: &WindowRef) {
        let looking = input.mouse_held(MouseButton::Right);
        if looking != self.looking {
            window.set_mouse_grab(looking);
            window.set_cursor_visible(!looking);
            self.looking = looking;
        }

        if looking {
            let motion = input.raw_motion();
            self.yaw -= motion.x * self.sensitivity;
            self.pitch = (self.pitch - motion.y * self.sensitivity).clamp(-1.5, 1.5);
        }

        let rotation = Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0);
        let forward = rotation * Vec3::NEG_Z;
        let right = rotation * Vec3::X;
        let mut direction = Vec3::ZERO;
        if input.key_held(KeyCode::KeyW) {
            direction += forward;
        }
        if input.key_held(KeyCode::KeyS) {
            direction -= forward;
        }
        if input.key_held(KeyCode::KeyD) {
            direction += right;
        }
        if input.key_held(KeyCode::KeyA) {
            direction -= right;
        }
        if input.key_held(KeyCode::Space) {
            direction += Vec3::Y;
        }
        if input.key_held(KeyCode::ControlLeft) {
            direction -= Vec3::Y;
        }

        let speed = if input.key_held(KeyCode::ShiftLeft) {
            self.speed * 4.0
        } else {
            self.speed
        };
        if direction.length_squared() > 0.0 {
            self.position += direction.normalize() * speed * dt;
        }
    }
}
