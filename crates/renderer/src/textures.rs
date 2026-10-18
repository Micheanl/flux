use wgpu::{
    Device, Extent3d, Origin3d, Queue, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor, TextureViewDimension,
};

use crate::config::RendererConfig;
use crate::mesh::{DEFAULT_TEXTURE_LAYERS, TextureData};

pub struct TextureArrays {
    pub albedo: TextureView,
    pub normal: TextureView,
    pub mr: TextureView,
    pub emissive: TextureView,
}

pub fn upload_textures(
    device: &Device,
    queue: &Queue,
    config: &RendererConfig,
    textures: &[TextureData],
) -> TextureArrays {
    let size = config.texture_size.max(4);
    let mip_levels = (size as f32).log2().floor() as u32 + 1;
    let layers = config
        .texture_layers
        .max(textures.len() as u32 + DEFAULT_TEXTURE_LAYERS)
        .max(DEFAULT_TEXTURE_LAYERS);

    let albedo = create_array(device, "albedo", size, mip_levels, layers, true);
    let normal = create_array(device, "normal", size, mip_levels, layers, false);
    let mr = create_array(
        device,
        "metallic roughness",
        size,
        mip_levels,
        layers,
        false,
    );
    let emissive = create_array(device, "emissive", size, mip_levels, layers, true);

    let albedo_fallback = TextureData::solid([255, 255, 255, 255]);
    let normal_fallback = TextureData::solid([128, 128, 255, 255]);
    let mr_fallback = TextureData::solid([255, 128, 0, 255]);
    let emissive_fallback = TextureData::solid([0, 0, 0, 255]);

    for layer in 0..layers {
        let source = layer
            .checked_sub(DEFAULT_TEXTURE_LAYERS)
            .and_then(|index| textures.get(index as usize));
        write_layer(
            queue,
            &albedo,
            layer,
            size,
            mip_levels,
            source.unwrap_or(&albedo_fallback),
        );
        write_layer(
            queue,
            &normal,
            layer,
            size,
            mip_levels,
            source.unwrap_or(&normal_fallback),
        );
        write_layer(
            queue,
            &mr,
            layer,
            size,
            mip_levels,
            source.unwrap_or(&mr_fallback),
        );
        write_layer(
            queue,
            &emissive,
            layer,
            size,
            mip_levels,
            source.unwrap_or(&emissive_fallback),
        );
    }

    let view = |texture: &Texture| {
        texture.create_view(&TextureViewDescriptor {
            label: Some("texture arrays"),
            dimension: Some(TextureViewDimension::D2Array),
            ..Default::default()
        })
    };

    TextureArrays {
        albedo: view(&albedo),
        normal: view(&normal),
        mr: view(&mr),
        emissive: view(&emissive),
    }
}

fn create_array(
    device: &Device,
    label: &str,
    size: u32,
    mip_levels: u32,
    layers: u32,
    srgb: bool,
) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        mip_level_count: mip_levels,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: if srgb {
            TextureFormat::Rgba8UnormSrgb
        } else {
            TextureFormat::Rgba8Unorm
        },
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn write_layer(
    queue: &Queue,
    texture: &Texture,
    layer: u32,
    size: u32,
    mip_levels: u32,
    source: &TextureData,
) {
    let mut level = (size, size, resize(source, size));
    for mip in 0..mip_levels {
        queue.write_texture(
            TexelCopyTextureInfo {
                texture,
                mip_level: mip,
                origin: Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &level.2,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(level.0 * 4),
                rows_per_image: Some(level.1),
            },
            Extent3d {
                width: level.0,
                height: level.1,
                depth_or_array_layers: 1,
            },
        );
        if mip + 1 < mip_levels {
            level = downsample(level.0, level.1, &level.2);
        }
    }
}

fn resize(source: &TextureData, size: u32) -> Vec<u8> {
    if source.width == size && source.height == size {
        return source.rgba.clone();
    }
    let mut output = vec![0u8; (size * size * 4) as usize];
    let scale_x = source.width as f32 / size as f32;
    let scale_y = source.height as f32 / size as f32;
    for y in 0..size {
        let sample_y = ((y as f32 + 0.5) * scale_y - 0.5).max(0.0);
        let y0 = sample_y.floor() as u32;
        let y1 = (y0 + 1).min(source.height.saturating_sub(1));
        let fy = (sample_y - y0 as f32).clamp(0.0, 1.0);
        for x in 0..size {
            let sample_x = ((x as f32 + 0.5) * scale_x - 0.5).max(0.0);
            let x0 = sample_x.floor() as u32;
            let x1 = (x0 + 1).min(source.width.saturating_sub(1));
            let fx = (sample_x - x0 as f32).clamp(0.0, 1.0);
            let mut pixel = [0u8; 4];
            for (channel, value) in pixel.iter_mut().enumerate() {
                let top = lerp(
                    texel(source, x0, y0, channel),
                    texel(source, x1, y0, channel),
                    fx,
                );
                let bottom = lerp(
                    texel(source, x0, y1, channel),
                    texel(source, x1, y1, channel),
                    fx,
                );
                *value = lerp(top, bottom, fy).round() as u8;
            }
            let offset = ((y * size + x) * 4) as usize;
            output[offset..offset + 4].copy_from_slice(&pixel);
        }
    }
    output
}

fn texel(source: &TextureData, x: u32, y: u32, channel: usize) -> f32 {
    let index = ((y.min(source.height - 1) * source.width + x.min(source.width - 1)) * 4) as usize;
    source.rgba[index + channel] as f32
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn downsample(width: u32, height: u32, rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    let next_width = (width / 2).max(1);
    let next_height = (height / 2).max(1);
    let mut output = vec![0u8; (next_width * next_height * 4) as usize];
    for y in 0..next_height {
        for x in 0..next_width {
            let mut sum = [0u32; 4];
            for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                let source_x = (x * 2 + dx).min(width - 1);
                let source_y = (y * 2 + dy).min(height - 1);
                let index = ((source_y * width + source_x) * 4) as usize;
                for channel in 0..4 {
                    sum[channel] += rgba[index + channel] as u32;
                }
            }
            let offset = ((y * next_width + x) * 4) as usize;
            for channel in 0..4 {
                output[offset + channel] = (sum[channel] / 4) as u8;
            }
        }
    }
    (next_width, next_height, output)
}
