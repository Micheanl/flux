mod app;
mod camera;
mod config;
mod engine;

pub use app::{App, run};
pub use camera::FlyCamera;
pub use config::EngineConfig;
pub use engine::Engine;

pub use foundation::{Aabb, EulerRot, FrameTime, Mat4, Quat, Transform, Vec2, Vec3, Vec4};
pub use platform::{Flow, Input, KeyCode, MouseButton, WindowRef};
pub use renderer::{
    MaterialData, MeshData, RenderSettings, RendererConfig, TextureData, Vertex, checker_texture,
    cube, plane, torus, uv_sphere,
};
pub use scene::{DirectionalLight, Entity, GlobalTransform, Name, PointLight, Scene};
