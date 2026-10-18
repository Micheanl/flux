use bytemuck::{Pod, Zeroable};
use foundation::{Aabb, Camera, Mat4, Vec3, Vec4};

use crate::mesh::{DEFAULT_TEXTURE_LAYERS, MaterialData, MeshData, TextureData};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct InstanceData {
    pub model: Mat4,
    pub bounds_min: Vec4,
    pub bounds_max: Vec4,
    pub mesh: u32,
    pub material: u32,
    pub flags: u32,
    pub _pad: u32,
}

impl InstanceData {
    pub fn new(model: Mat4, local_bounds: &Aabb, mesh: u32, material: u32) -> Self {
        let bounds = local_bounds.transform(&model);
        Self {
            model,
            bounds_min: bounds.min.extend(0.0),
            bounds_max: bounds.max.extend(0.0),
            mesh,
            material,
            flags: 0,
            _pad: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CameraData {
    pub view_projection: Mat4,
    pub position: Vec3,
    pub near: f32,
}

impl Default for CameraData {
    fn default() -> Self {
        Self {
            view_projection: Mat4::IDENTITY,
            position: Vec3::ZERO,
            near: 0.1,
        }
    }
}

impl CameraData {
    pub fn from_camera(camera: &Camera) -> Self {
        Self {
            view_projection: camera.view_projection,
            position: camera.position,
            near: camera.near,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SunLight {
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
}

impl Default for SunLight {
    fn default() -> Self {
        Self {
            direction: Vec3::new(0.35, 0.85, 0.4).normalize(),
            color: Vec3::new(1.0, 0.95, 0.85),
            intensity: 5.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PointLight {
    pub position: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub radius: f32,
}

impl PointLight {
    pub fn new(position: Vec3, color: Vec3, intensity: f32) -> Self {
        Self {
            position,
            color,
            intensity,
            radius: 12.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RenderSettings {
    pub exposure: f32,
    pub bloom: f32,
    pub shadow_distance: f32,
    pub ambient: f32,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            exposure: 1.0,
            bloom: 0.08,
            shadow_distance: 120.0,
            ambient: 0.15,
        }
    }
}

#[derive(Default)]
pub struct RenderScene {
    pub revision: u64,
    pub camera: CameraData,
    pub sun: SunLight,
    pub point_lights: Vec<PointLight>,
    pub meshes: Vec<MeshData>,
    pub materials: Vec<MaterialData>,
    pub textures: Vec<TextureData>,
    pub instances: Vec<InstanceData>,
    pub settings: RenderSettings,
}

impl RenderScene {
    pub fn mark_dirty(&mut self) {
        self.revision += 1;
    }

    pub fn add_mesh(&mut self, mesh: MeshData) -> u32 {
        self.meshes.push(mesh);
        self.mark_dirty();
        (self.meshes.len() - 1) as u32
    }

    pub fn add_material(&mut self, material: MaterialData) -> u32 {
        self.materials.push(material);
        self.mark_dirty();
        (self.materials.len() - 1) as u32
    }

    pub fn add_texture(&mut self, texture: TextureData) -> u32 {
        self.textures.push(texture);
        self.mark_dirty();
        DEFAULT_TEXTURE_LAYERS + (self.textures.len() - 1) as u32
    }
}
