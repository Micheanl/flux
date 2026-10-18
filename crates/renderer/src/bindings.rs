use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, Buffer,
    BufferBinding, BufferDescriptor, BufferUsages, Device, Queue, TextureView,
};

use crate::gpu_scene::GpuScene;
use crate::passes::Passes;
use crate::targets::Targets;
use crate::uniforms::{BloomUniform, COUNT_STRIDE, VIEW_COUNT, VIEW_UNIFORM_STRIDE};

pub struct SceneBindings {
    pub textures: BindGroup,
    pub storage: BindGroup,
    pub cull_input: BindGroup,
    pub cull_output: Vec<BindGroup>,
}

pub struct TargetBindings {
    pub bloom_down: Vec<BindGroup>,
    pub bloom_upsample: Vec<BindGroup>,
    pub tonemap: BindGroup,
}

pub fn scene_bindings(
    device: &Device,
    passes: &Passes,
    scene: &GpuScene,
    targets: &Targets,
    draw_buffer: &Buffer,
    count_buffer: &Buffer,
) -> SceneBindings {
    let textures = device.create_bind_group(&BindGroupDescriptor {
        label: Some("texture bindings"),
        layout: &passes.textures_layout,
        entries: &[
            texture_entry(0, &scene.textures.albedo),
            texture_entry(1, &scene.textures.normal),
            texture_entry(2, &scene.textures.mr),
            texture_entry(3, &scene.textures.emissive),
            BindGroupEntry {
                binding: 4,
                resource: BindingResource::Sampler(&passes.linear_sampler),
            },
            BindGroupEntry {
                binding: 5,
                resource: BindingResource::TextureView(&targets.shadow_view),
            },
            BindGroupEntry {
                binding: 6,
                resource: BindingResource::Sampler(&passes.shadow_sampler),
            },
        ],
    });

    let storage = device.create_bind_group(&BindGroupDescriptor {
        label: Some("instance and material bindings"),
        layout: &passes.storage_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: scene.instance_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: scene.material_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: scene.light_buffer.as_entire_binding(),
            },
        ],
    });

    let cull_input = device.create_bind_group(&BindGroupDescriptor {
        label: Some("cull input bindings"),
        layout: &passes.cull_input_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: scene.instance_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: scene.mesh_buffer.as_entire_binding(),
            },
        ],
    });

    let draw_stride = scene.instance_capacity as u64 * 20;
    let cull_output = (0..VIEW_COUNT)
        .map(|view| {
            device.create_bind_group(&BindGroupDescriptor {
                label: Some("cull output bindings"),
                layout: &passes.cull_output_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: draw_buffer,
                            offset: view as u64 * draw_stride,
                            size: std::num::NonZeroU64::new(draw_stride),
                        }),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: count_buffer,
                            offset: view as u64 * COUNT_STRIDE,
                            size: std::num::NonZeroU64::new(4),
                        }),
                    },
                ],
            })
        })
        .collect();

    SceneBindings {
        textures,
        storage,
        cull_input,
        cull_output,
    }
}

pub fn target_bindings(
    device: &Device,
    queue: &Queue,
    passes: &Passes,
    targets: &Targets,
) -> TargetBindings {
    let bloom_bind_group =
        |source: &TextureView, source_size: (u32, u32), target_size: (u32, u32)| -> BindGroup {
            let uniform = BloomUniform {
                source_size: [source_size.0 as f32, source_size.1 as f32],
                target_size: [target_size.0 as f32, target_size.1 as f32],
            };
            let uniform_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("bloom size"),
                size: std::mem::size_of::<BloomUniform>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&uniform));

            device.create_bind_group(&BindGroupDescriptor {
                label: Some("bloom bindings"),
                layout: &passes.bloom_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(source),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&passes.linear_sampler),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            })
        };

    let bloom_down = std::iter::once(bloom_bind_group(
        &targets.hdr_view,
        targets.size,
        targets.bloom[0].size,
    ))
    .chain(
        targets
            .bloom
            .iter()
            .take(targets.bloom.len().saturating_sub(1))
            .enumerate()
            .map(|(index, source)| {
                bloom_bind_group(&source.view, source.size, targets.bloom[index + 1].size)
            }),
    )
    .collect();

    let bloom_upsample = targets
        .bloom
        .iter()
        .enumerate()
        .skip(1)
        .map(|(index, source)| {
            bloom_bind_group(&source.view, source.size, targets.bloom[index - 1].size)
        })
        .collect();

    let tonemap = {
        let bloom = &targets.bloom[0].view;
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("tonemap bindings"),
            layout: &passes.tonemap_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&targets.hdr_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(bloom),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&passes.linear_sampler),
                },
            ],
        })
    };

    TargetBindings {
        bloom_down,
        bloom_upsample,
        tonemap,
    }
}

pub fn frame_bind_group(device: &Device, layout: &BindGroupLayout, buffer: &Buffer) -> BindGroup {
    device.create_bind_group(&BindGroupDescriptor {
        label: Some("frame data bindings"),
        layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}

pub fn view_bind_group(device: &Device, layout: &BindGroupLayout, buffer: &Buffer) -> BindGroup {
    device.create_bind_group(&BindGroupDescriptor {
        label: Some("view data bindings"),
        layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(BufferBinding {
                buffer,
                offset: 0,
                size: std::num::NonZeroU64::new(VIEW_UNIFORM_STRIDE),
            }),
        }],
    })
}

fn texture_entry(binding: u32, view: &TextureView) -> BindGroupEntry<'_> {
    BindGroupEntry {
        binding,
        resource: BindingResource::TextureView(view),
    }
}
