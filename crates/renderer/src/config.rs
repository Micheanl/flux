use wgpu::{Features, TextureFormat};

#[derive(Clone, Debug)]
pub struct RendererConfig {
    pub shadow_map_size: u32,
    pub cascade_count: u32,
    pub max_instances: u32,
    pub texture_size: u32,
    pub texture_layers: u32,
    pub bloom_mips: u32,
    pub hdr_format: TextureFormat,
    pub gpu_driven: bool,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            shadow_map_size: 2048,
            cascade_count: 4,
            max_instances: 65_536,
            texture_size: 512,
            texture_layers: 16,
            bloom_mips: 5,
            hdr_format: TextureFormat::Rgba16Float,
            gpu_driven: true,
        }
    }
}

impl RendererConfig {
    pub fn required_features() -> Features {
        Features::MULTI_DRAW_INDIRECT_COUNT
    }
}
