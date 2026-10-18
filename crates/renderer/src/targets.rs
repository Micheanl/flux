use wgpu::{
    Device, Extent3d, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureView, TextureViewDescriptor, TextureViewDimension,
};

use crate::config::RendererConfig;

pub struct BloomTarget {
    pub view: TextureView,
    pub size: (u32, u32),
}

pub struct Targets {
    pub size: (u32, u32),
    pub hdr: Texture,
    pub hdr_view: TextureView,
    pub depth: Texture,
    pub depth_view: TextureView,
    pub bloom: Vec<BloomTarget>,
    pub shadow_view: TextureView,
    pub shadow_layers: Vec<TextureView>,
    pub offscreen: Option<Texture>,
    pub offscreen_view: Option<TextureView>,
}

impl Targets {
    pub fn new(
        device: &Device,
        config: &RendererConfig,
        output_format: TextureFormat,
        offscreen: bool,
    ) -> Self {
        let shadow = create_texture(
            device,
            "shadow map",
            (
                config.shadow_map_size.max(256),
                config.shadow_map_size.max(256),
            ),
            TextureFormat::Depth32Float,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            1,
            config.cascade_count.max(1),
        );
        let shadow_view = shadow.create_view(&TextureViewDescriptor {
            label: Some("shadow array view"),
            dimension: Some(TextureViewDimension::D2Array),
            ..Default::default()
        });
        let shadow_layers = (0..config.cascade_count.max(1))
            .map(|layer| {
                shadow.create_view(&TextureViewDescriptor {
                    label: Some("shadow layer"),
                    dimension: Some(TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();

        let hdr = create_texture(
            device,
            "hdr color",
            (1, 1),
            config.hdr_format,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            1,
            1,
        );
        let hdr_view = hdr.create_view(&TextureViewDescriptor::default());
        let depth = create_texture(
            device,
            "main depth",
            (1, 1),
            TextureFormat::Depth32Float,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            1,
            1,
        );
        let depth_view = depth.create_view(&TextureViewDescriptor::default());

        let mut targets = Self {
            size: (0, 0),
            hdr,
            hdr_view,
            depth,
            depth_view,
            bloom: Vec::new(),
            shadow_view,
            shadow_layers,
            offscreen: None,
            offscreen_view: None,
        };
        targets.resize(device, config, output_format, (1, 1), offscreen);
        targets
    }

    pub fn resize(
        &mut self,
        device: &Device,
        config: &RendererConfig,
        output_format: TextureFormat,
        size: (u32, u32),
        offscreen: bool,
    ) -> bool {
        let size = (size.0.max(1), size.1.max(1));
        let rebuild = self.size != size;
        self.size = size;

        if rebuild {
            self.hdr = create_texture(
                device,
                "hdr color",
                size,
                config.hdr_format,
                TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                1,
                1,
            );
            self.hdr_view = self.hdr.create_view(&TextureViewDescriptor::default());
            self.depth = create_texture(
                device,
                "main depth",
                size,
                TextureFormat::Depth32Float,
                TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                1,
                1,
            );
            self.depth_view = self.depth.create_view(&TextureViewDescriptor::default());

            let mut bloom_size = ((size.0 / 2).max(1), (size.1 / 2).max(1));
            self.bloom = (0..config.bloom_mips.max(1))
                .map(|_| {
                    let texture = create_texture(
                        device,
                        "bloom",
                        bloom_size,
                        config.hdr_format,
                        TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                        1,
                        1,
                    );
                    let target = BloomTarget {
                        view: texture.create_view(&TextureViewDescriptor::default()),
                        size: bloom_size,
                    };
                    bloom_size = ((bloom_size.0 / 2).max(1), (bloom_size.1 / 2).max(1));
                    target
                })
                .collect();
        }

        if offscreen && self.offscreen.is_none() {
            let texture = create_texture(
                device,
                "offscreen color",
                size,
                output_format,
                TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
                1,
                1,
            );
            self.offscreen_view = Some(texture.create_view(&TextureViewDescriptor::default()));
            self.offscreen = Some(texture);
        } else if !offscreen {
            self.offscreen = None;
            self.offscreen_view = None;
        }

        rebuild
    }

    pub fn shadow_layer(&self, index: usize) -> &TextureView {
        &self.shadow_layers[index.min(self.shadow_layers.len() - 1)]
    }
}

fn create_texture(
    device: &Device,
    label: &str,
    size: (u32, u32),
    format: TextureFormat,
    usage: TextureUsages,
    mip_levels: u32,
    array_layers: u32,
) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: array_layers,
        },
        mip_level_count: mip_levels,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}
