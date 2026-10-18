use bytemuck::{Pod, Zeroable};
use foundation::{Aabb, Vec3, Vec4};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub tangent: [f32; 4],
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}

impl Vertex {
    pub fn new(position: [f32; 3], normal: [f32; 3], uv: [f32; 2], tangent: [f32; 4]) -> Self {
        Self {
            position,
            normal,
            uv,
            tangent,
            joints: [0; 4],
            weights: [0.0; 4],
        }
    }

    pub const ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x2,
        3 => Float32x4,
        4 => Uint16x4,
        5 => Float32x4,
    ];

    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &Self::ATTRIBUTES,
    };
}

#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub bounds: Aabb,
}

impl MeshData {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        let bounds = Aabb::from_points(
            vertices
                .iter()
                .map(|vertex| Vec3::from_array(vertex.position)),
        );
        Self {
            vertices,
            indices,
            bounds: if bounds.is_valid() {
                bounds
            } else {
                Aabb::new(Vec3::ZERO, Vec3::ZERO)
            },
        }
    }
}

pub const DEFAULT_TEXTURE_LAYERS: u32 = 4;
pub const TEXTURE_WHITE: u32 = 0;
pub const TEXTURE_FLAT_NORMAL: u32 = 1;
pub const TEXTURE_DEFAULT_MR: u32 = 2;
pub const TEXTURE_BLACK: u32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MaterialData {
    pub base_color: Vec4,
    pub emissive_metallic: Vec4,
    pub roughness_uv_scale: Vec4,
    pub uv_offset_flags: Vec4,
    pub textures: [u32; 4],
}

impl Default for MaterialData {
    fn default() -> Self {
        Self::pbr(Vec4::new(0.8, 0.8, 0.8, 1.0), 0.0, 0.5)
    }
}

impl MaterialData {
    pub fn pbr(base_color: Vec4, metallic: f32, roughness: f32) -> Self {
        Self {
            base_color,
            emissive_metallic: Vec4::new(0.0, 0.0, 0.0, metallic),
            roughness_uv_scale: Vec4::new(roughness, 1.0, 1.0, 0.0),
            uv_offset_flags: Vec4::ZERO,
            textures: [
                TEXTURE_WHITE,
                TEXTURE_FLAT_NORMAL,
                TEXTURE_DEFAULT_MR,
                TEXTURE_BLACK,
            ],
        }
    }

    pub fn with_textures(mut self, albedo: u32, normal: u32, mr: u32, emissive: u32) -> Self {
        self.textures = [albedo, normal, mr, emissive];
        self
    }

    pub fn with_uv_scale(mut self, scale: [f32; 2], offset: [f32; 2]) -> Self {
        self.roughness_uv_scale.y = scale[0];
        self.roughness_uv_scale.z = scale[1];
        self.uv_offset_flags.x = offset[0];
        self.uv_offset_flags.y = offset[1];
        self
    }
}

#[derive(Clone, Debug)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl TextureData {
    pub fn solid(color: [u8; 4]) -> Self {
        Self {
            width: 1,
            height: 1,
            rgba: color.to_vec(),
        }
    }

    pub fn from_rgba8(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        assert_eq!(
            rgba.len() as u32,
            width * height * 4,
            "invalid RGBA data length"
        );
        Self {
            width,
            height,
            rgba,
        }
    }
}

pub fn checker_texture(size: u32, cell: u32, first: [u8; 4], second: [u8; 4]) -> TextureData {
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let color = if ((x / cell) + (y / cell)).is_multiple_of(2) {
                first
            } else {
                second
            };
            rgba.extend_from_slice(&color);
        }
    }
    TextureData {
        width: size,
        height: size,
        rgba,
    }
}
