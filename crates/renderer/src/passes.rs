use std::num::NonZeroU64;

use wgpu::*;

use crate::config::RendererConfig;
use crate::mesh::Vertex;
use crate::shaders;
use crate::uniforms::VIEW_UNIFORM_STRIDE;

const DEPTH_ATTRIBUTES: [VertexAttribute; 4] = [
    VertexAttribute {
        format: VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    },
    VertexAttribute {
        format: VertexFormat::Float32x2,
        offset: std::mem::offset_of!(Vertex, uv) as u64,
        shader_location: 2,
    },
    VertexAttribute {
        format: VertexFormat::Uint16x4,
        offset: std::mem::offset_of!(Vertex, joints) as u64,
        shader_location: 4,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: std::mem::offset_of!(Vertex, weights) as u64,
        shader_location: 5,
    },
];

const DEPTH_LAYOUT: VertexBufferLayout<'static> = VertexBufferLayout {
    array_stride: std::mem::size_of::<Vertex>() as u64,
    step_mode: VertexStepMode::Vertex,
    attributes: &DEPTH_ATTRIBUTES,
};

pub struct Passes {
    pub frame_layout: BindGroupLayout,
    pub view_layout: BindGroupLayout,
    pub textures_layout: BindGroupLayout,
    pub depth_textures_layout: BindGroupLayout,
    pub storage_layout: BindGroupLayout,
    pub cull_input_layout: BindGroupLayout,
    pub cull_output_layout: BindGroupLayout,
    pub bloom_layout: BindGroupLayout,
    pub tonemap_layout: BindGroupLayout,
    pub skin_layout: BindGroupLayout,

    pub mesh_pipeline: RenderPipeline,
    pub blend_pipeline: RenderPipeline,
    pub depth_pipeline: RenderPipeline,
    pub shadow_pipeline: RenderPipeline,
    pub sky_pipeline: RenderPipeline,
    pub cull_pipeline: ComputePipeline,
    pub prefilter_pipeline: RenderPipeline,
    pub downsample_pipeline: RenderPipeline,
    pub upsample_pipeline: RenderPipeline,
    pub tonemap_pipeline: RenderPipeline,

    pub linear_sampler: Sampler,
    pub shadow_sampler: Sampler,
}

impl Passes {
    pub fn new(device: &Device, config: &RendererConfig, output_format: TextureFormat) -> Self {
        let frame_layout =
            buffer_layout(device, "frame data", ShaderStages::VERTEX_FRAGMENT, false);
        let view_layout = buffer_layout(
            device,
            "view data",
            ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
            true,
        );

        let textures_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("textures"),
            entries: &[
                texture_entry(
                    0,
                    TextureSampleType::Float { filterable: true },
                    TextureViewDimension::D2Array,
                ),
                texture_entry(
                    1,
                    TextureSampleType::Float { filterable: true },
                    TextureViewDimension::D2Array,
                ),
                texture_entry(
                    2,
                    TextureSampleType::Float { filterable: true },
                    TextureViewDimension::D2Array,
                ),
                texture_entry(
                    3,
                    TextureSampleType::Float { filterable: true },
                    TextureViewDimension::D2Array,
                ),
                sampler_entry(4, SamplerBindingType::Filtering),
                texture_entry(5, TextureSampleType::Depth, TextureViewDimension::D2Array),
                sampler_entry(6, SamplerBindingType::Comparison),
            ],
        });

        let depth_textures_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("depth textures"),
            entries: &[
                texture_entry(
                    0,
                    TextureSampleType::Float { filterable: true },
                    TextureViewDimension::D2Array,
                ),
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let storage_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("instances and materials"),
            entries: &[
                storage_entry(0, ShaderStages::VERTEX_FRAGMENT, true),
                storage_entry(1, ShaderStages::FRAGMENT, true),
                storage_entry(2, ShaderStages::FRAGMENT, true),
            ],
        });

        let cull_input_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("cull input"),
            entries: &[
                storage_entry(0, ShaderStages::COMPUTE, true),
                storage_entry(1, ShaderStages::COMPUTE, true),
                storage_entry(2, ShaderStages::COMPUTE, true),
            ],
        });

        let cull_output_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("cull output"),
            entries: &[
                storage_entry(0, ShaderStages::COMPUTE, false),
                storage_entry(1, ShaderStages::COMPUTE, false),
            ],
        });

        let bloom_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("bloom input"),
            entries: &[
                texture_entry(
                    0,
                    TextureSampleType::Float { filterable: true },
                    TextureViewDimension::D2,
                ),
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                sampler_entry(1, SamplerBindingType::Filtering),
            ],
        });

        let tonemap_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("tonemap input"),
            entries: &[
                texture_entry(
                    0,
                    TextureSampleType::Float { filterable: true },
                    TextureViewDimension::D2,
                ),
                texture_entry(
                    1,
                    TextureSampleType::Float { filterable: true },
                    TextureViewDimension::D2,
                ),
                sampler_entry(2, SamplerBindingType::Filtering),
            ],
        });

        let mesh_module = shader_module(device, "mesh");
        let depth_module = shader_module(device, "depth");
        let cull_module = shader_module(device, "cull");
        let sky_module = shader_module(device, "sky");
        let bloom_module = shader_module(device, "bloom");
        let tonemap_module = shader_module(device, "tonemap");

        let skin_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("skin"),
            entries: &[storage_entry(0, ShaderStages::VERTEX, true)],
        });

        let mesh_pipeline_layout = pipeline_layout(
            device,
            "opaque layout",
            &[
                &frame_layout,
                &textures_layout,
                &storage_layout,
                &skin_layout,
            ],
        );
        let depth_pipeline_layout = pipeline_layout(
            device,
            "depth layout",
            &[
                &view_layout,
                &depth_textures_layout,
                &storage_layout,
                &skin_layout,
            ],
        );
        let sky_pipeline_layout = pipeline_layout(device, "sky layout", &[&frame_layout]);
        let cull_pipeline_layout = pipeline_layout(
            device,
            "cull layout",
            &[&view_layout, &cull_input_layout, &cull_output_layout],
        );
        let bloom_pipeline_layout = pipeline_layout(device, "bloom layout", &[&bloom_layout]);
        let tonemap_pipeline_layout =
            pipeline_layout(device, "tonemap layout", &[&frame_layout, &tonemap_layout]);

        let mesh_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("opaque pbr"),
            layout: Some(&mesh_pipeline_layout),
            vertex: VertexState {
                module: &mesh_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(Vertex::LAYOUT)],
            },
            primitive: PrimitiveState {
                cull_mode: None,
                ..default_primitive()
            },
            depth_stencil: Some(depth_state(
                CompareFunction::Equal,
                false,
                DepthBiasState::default(),
            )),
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &mesh_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: config.hdr_format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let blend_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("transparent pbr"),
            layout: Some(&mesh_pipeline_layout),
            vertex: VertexState {
                module: &mesh_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(Vertex::LAYOUT)],
            },
            primitive: PrimitiveState {
                cull_mode: None,
                ..default_primitive()
            },
            depth_stencil: Some(depth_state(
                CompareFunction::GreaterEqual,
                false,
                DepthBiasState::default(),
            )),
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &mesh_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: config.hdr_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let depth_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("depth prepass"),
            layout: Some(&depth_pipeline_layout),
            vertex: VertexState {
                module: &depth_module,
                entry_point: Some("vs_depth"),
                compilation_options: Default::default(),
                buffers: &[Some(DEPTH_LAYOUT)],
            },
            primitive: PrimitiveState {
                cull_mode: None,
                ..default_primitive()
            },
            depth_stencil: Some(depth_state(
                CompareFunction::GreaterEqual,
                true,
                DepthBiasState::default(),
            )),
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &depth_module,
                entry_point: Some("fs_depth"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            multiview_mask: None,
            cache: None,
        });

        let shadow_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&depth_pipeline_layout),
            vertex: VertexState {
                module: &depth_module,
                entry_point: Some("vs_depth"),
                compilation_options: Default::default(),
                buffers: &[Some(DEPTH_LAYOUT)],
            },
            primitive: PrimitiveState {
                cull_mode: None,
                ..default_primitive()
            },
            depth_stencil: Some(depth_state(
                CompareFunction::GreaterEqual,
                true,
                DepthBiasState {
                    constant: 2,
                    slope_scale: 1.75,
                    clamp: 0.0,
                },
            )),
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &depth_module,
                entry_point: Some("fs_depth"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sky_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&sky_pipeline_layout),
            vertex: VertexState {
                module: &sky_module,
                entry_point: Some("vs_fullscreen"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: PrimitiveState {
                cull_mode: None,
                ..default_primitive()
            },
            depth_stencil: Some(depth_state(
                CompareFunction::GreaterEqual,
                false,
                DepthBiasState::default(),
            )),
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &sky_module,
                entry_point: Some("fs_sky"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: config.hdr_format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let prefilter_pipeline = bloom_pipeline(
            device,
            "bloom prefilter",
            &bloom_pipeline_layout,
            &bloom_module,
            "fs_prefilter",
            config.hdr_format,
            None,
        );
        let downsample_pipeline = bloom_pipeline(
            device,
            "bloom downsample",
            &bloom_pipeline_layout,
            &bloom_module,
            "fs_downsample",
            config.hdr_format,
            None,
        );
        let upsample_pipeline = bloom_pipeline(
            device,
            "bloom upsample",
            &bloom_pipeline_layout,
            &bloom_module,
            "fs_upsample",
            config.hdr_format,
            Some(BlendState::ADDITIVE),
        );

        let tonemap_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("tonemap"),
            layout: Some(&tonemap_pipeline_layout),
            vertex: VertexState {
                module: &tonemap_module,
                entry_point: Some("vs_fullscreen"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: PrimitiveState {
                cull_mode: None,
                ..default_primitive()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &tonemap_module,
                entry_point: Some("fs_tonemap"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: output_format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let cull_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("frustum culling"),
            layout: Some(&cull_pipeline_layout),
            module: &cull_module,
            entry_point: Some("cull_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let linear_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("linear sampler"),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });

        let shadow_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("shadow sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            compare: Some(CompareFunction::GreaterEqual),
            anisotropy_clamp: 1,
            border_color: None,
        });

        Self {
            frame_layout,
            view_layout,
            textures_layout,
            depth_textures_layout,
            storage_layout,
            cull_input_layout,
            cull_output_layout,
            bloom_layout,
            tonemap_layout,
            skin_layout,
            mesh_pipeline,
            blend_pipeline,
            depth_pipeline,
            shadow_pipeline,
            sky_pipeline,
            cull_pipeline,
            prefilter_pipeline,
            downsample_pipeline,
            upsample_pipeline,
            tonemap_pipeline,
            linear_sampler,
            shadow_sampler,
        }
    }
}

fn shader_module(device: &Device, name: &str) -> ShaderModule {
    device.create_shader_module(ShaderModuleDescriptor {
        label: Some(name),
        source: ShaderSource::Wgsl(shaders::module(name).into()),
    })
}

fn pipeline_layout(device: &Device, label: &str, groups: &[&BindGroupLayout]) -> PipelineLayout {
    let groups: Vec<Option<&BindGroupLayout>> = groups.iter().map(|group| Some(*group)).collect();
    device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &groups,
        immediate_size: 0,
    })
}

fn buffer_layout(
    device: &Device,
    label: &str,
    visibility: ShaderStages,
    dynamic: bool,
) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: dynamic,
                min_binding_size: if dynamic {
                    NonZeroU64::new(VIEW_UNIFORM_STRIDE)
                } else {
                    None
                },
            },
            count: None,
        }],
    })
}

fn texture_entry(
    binding: u32,
    sample_type: TextureSampleType,
    view_dimension: TextureViewDimension,
) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type,
            view_dimension,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32, ty: SamplerBindingType) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Sampler(ty),
        count: None,
    }
}

fn storage_entry(binding: u32, visibility: ShaderStages, read_only: bool) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn default_primitive() -> PrimitiveState {
    PrimitiveState {
        topology: PrimitiveTopology::TriangleList,
        strip_index_format: None,
        front_face: FrontFace::Ccw,
        cull_mode: Some(Face::Back),
        unclipped_depth: false,
        polygon_mode: PolygonMode::Fill,
        conservative: false,
    }
}

fn depth_state(compare: CompareFunction, write: bool, bias: DepthBiasState) -> DepthStencilState {
    DepthStencilState {
        format: TextureFormat::Depth32Float,
        depth_write_enabled: Some(write),
        depth_compare: Some(compare),
        stencil: StencilState::default(),
        bias,
    }
}

fn bloom_pipeline(
    device: &Device,
    label: &str,
    layout: &PipelineLayout,
    module: &ShaderModule,
    entry_point: &str,
    format: TextureFormat,
    blend: Option<BlendState>,
) -> RenderPipeline {
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: VertexState {
            module,
            entry_point: Some("vs_fullscreen"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: PrimitiveState {
            cull_mode: None,
            ..default_primitive()
        },
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            module,
            entry_point: Some(entry_point),
            compilation_options: Default::default(),
            targets: &[Some(ColorTargetState {
                format,
                blend,
                write_mask: ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
