use ecs::Entity;
pub use foundation::Transform;
use foundation::{Mat4, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct GlobalTransform(pub Mat4);

impl Default for GlobalTransform {
    fn default() -> Self {
        Self(Mat4::IDENTITY)
    }
}

impl GlobalTransform {
    pub fn forward(&self) -> Vec3 {
        -self.0.z_axis.truncate().normalize_or_zero()
    }

    pub fn translation(&self) -> Vec3 {
        self.0.w_axis.truncate()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MeshRenderer {
    pub mesh: u32,
    pub material: u32,
}

impl MeshRenderer {
    pub fn new(mesh: u32, material: u32) -> Self {
        Self { mesh, material }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CameraComponent {
    pub fov_y: f32,
    pub near: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DirectionalLight {
    pub color: Vec3,
    pub intensity: f32,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            color: Vec3::new(1.0, 0.95, 0.86),
            intensity: 4.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PointLight {
    pub color: Vec3,
    pub intensity: f32,
    pub radius: f32,
}

impl PointLight {
    pub fn new(color: Vec3, intensity: f32, radius: f32) -> Self {
        Self {
            color,
            intensity,
            radius,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Parent(pub Entity);

#[derive(Clone, Debug, Default)]
pub struct Children(pub Vec<Entity>);

#[derive(Clone, Debug, Default)]
pub struct Name(pub String);
