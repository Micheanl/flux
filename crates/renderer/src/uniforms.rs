use bytemuck::{Pod, Zeroable};
use foundation::{Frustum, MAX_SHADOW_CASCADES, Mat4, Vec4};

pub const MAX_CASCADES: usize = MAX_SHADOW_CASCADES;
pub const VIEW_COUNT: usize = 1 + MAX_CASCADES + 1;
pub const VIEW_UNIFORM_STRIDE: u64 = 256;
pub const VIEW_UNIFORM_SIZE: usize = 256;
pub const COUNT_STRIDE: u64 = 256;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FrameUniform {
    pub view_proj: Mat4,
    pub inv_view_proj: Mat4,
    pub camera_pos: Vec4,
    pub sun_dir: Vec4,
    pub sun_color: Vec4,
    pub cascade_splits: Vec4,
    pub shadow_texel: Vec4,
    pub cascade_vp: [Mat4; MAX_CASCADES],
    pub exposure: Vec4,
    pub params: Vec4,
    pub viewport: Vec4,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct BloomUniform {
    pub source_size: [f32; 2],
    pub target_size: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ViewUniform {
    pub view_proj: Mat4,
    pub planes: [Vec4; 6],
    pub counts: [u32; 4],
    pub _pad: [Vec4; 5],
}

impl ViewUniform {
    pub fn new(view_proj: Mat4, instance_count: u32, filter: u32) -> Self {
        let frustum = Frustum::from_view_projection(&view_proj);
        let mut planes = [Vec4::ZERO; 6];
        for (slot, plane) in planes.iter_mut().zip(frustum.planes.iter()) {
            *slot = plane.normal.extend(plane.d);
        }
        Self {
            view_proj,
            planes,
            counts: [instance_count, filter, 0, 0],
            _pad: [Vec4::ZERO; 5],
        }
    }
}
