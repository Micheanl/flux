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
    pub skin_offset: u32,
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
            skin_offset: 0,
        }
    }

    pub fn skinned(
        model: Mat4,
        local_bounds: &Aabb,
        mesh: u32,
        material: u32,
        skin_offset: u32,
    ) -> Self {
        let bounds = local_bounds.transform(&model);
        let expanded =
            Aabb::from_center_extents(bounds.center(), bounds.extents() * 2.0 + Vec3::splat(0.5));
        Self {
            model,
            bounds_min: expanded.min.extend(0.0),
            bounds_max: expanded.max.extend(0.0),
            mesh,
            material,
            flags: 1,
            skin_offset,
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
pub struct PunctualLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub radius: f32,
    pub inner_cos: f32,
    pub outer_cos: f32,
    pub spot: bool,
}

impl PunctualLight {
    pub fn point(position: Vec3, color: Vec3, intensity: f32, radius: f32) -> Self {
        Self {
            position,
            direction: Vec3::ZERO,
            color,
            intensity,
            radius,
            inner_cos: 1.0,
            outer_cos: 0.0,
            spot: false,
        }
    }

    pub fn spot(
        position: Vec3,
        direction: Vec3,
        color: Vec3,
        intensity: f32,
        radius: f32,
        inner_angle: f32,
        outer_angle: f32,
    ) -> Self {
        Self {
            position,
            direction: direction.normalize_or_zero(),
            color,
            intensity,
            radius,
            inner_cos: inner_angle.cos(),
            outer_cos: outer_angle.cos(),
            spot: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RenderSettings {
    pub exposure: f32,
    pub bloom: f32,
    pub shadow_distance: f32,
    pub ambient: f32,
    pub toon: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            exposure: 1.0,
            bloom: 0.08,
            shadow_distance: 120.0,
            ambient: 0.15,
            toon: false,
        }
    }
}

#[derive(Default)]
pub struct RenderScene {
    pub revision: u64,
    pub camera: CameraData,
    pub sun: SunLight,
    pub lights: Vec<PunctualLight>,
    pub meshes: Vec<MeshData>,
    pub materials: Vec<MaterialData>,
    pub textures: Vec<TextureData>,
    pub instances: Vec<InstanceData>,
    pub joint_matrices: Vec<Mat4>,
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
