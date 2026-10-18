use std::sync::mpsc;

use foundation::Mat4;
use wgpu::*;

use crate::bindings::{self, SceneBindings, TargetBindings};
use crate::cascades::{self, Cascades};
use crate::config::RendererConfig;
use crate::gpu_scene::GpuScene;
use crate::passes::Passes;
use crate::scene::RenderScene;
use crate::targets::Targets;
use crate::uniforms::{
    COUNT_STRIDE, FrameUniform, MAX_CASCADES, VIEW_COUNT, VIEW_UNIFORM_SIZE, VIEW_UNIFORM_STRIDE,
    ViewUniform,
};

const INITIAL_JOINT_CAPACITY: usize = 16384;
const TRANSPARENT_VIEW: usize = 1 + MAX_CASCADES;

pub struct RenderStats {
    pub frame_index: u64,
    pub instances: u32,
    pub draw_calls: u32,
    pub gpu_driven: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderOutcome {
    Presented,
    Skipped,
    Outdated,
    Lost,
}

pub struct Renderer {
    device: Device,
    queue: Queue,
    config: RendererConfig,
    output_format: TextureFormat,
    passes: Passes,
    gpu_scene: GpuScene,
    gpu_driven: bool,
    frame_buffer: Buffer,
    view_buffer: Buffer,
    frame_bind_group: BindGroup,
    view_bind_group: BindGroup,
    scene_bindings: SceneBindings,
    depth_textures_bind_group: BindGroup,
    target_bindings: TargetBindings,
    draw_buffer: Buffer,
    count_buffer: Buffer,
    joint_buffer: Buffer,
    joint_bind_group: BindGroup,
    joint_capacity: usize,
    targets: Targets,
    cascades: Cascades,
    frame_index: u64,
}

impl Renderer {
    pub fn new(gpu: &graphics::Gpu, config: RendererConfig, output_format: TextureFormat) -> Self {
        let device = gpu.device.clone();
        let queue = gpu.queue.clone();
        let gpu_driven =
            config.gpu_driven && gpu.features.contains(Features::MULTI_DRAW_INDIRECT_COUNT);
        if !gpu_driven {
            log::warn!("GPU culling unavailable, using per-instance draws");
        }

        let passes = Passes::new(&device, &config, output_format);
        let seed_scene = RenderScene::default();
        let gpu_scene = GpuScene::upload(&device, &queue, &config, &seed_scene);
        let targets = Targets::new(&device, &config, output_format, false);

        let frame_buffer = create_uniform_buffer(
            &device,
            "frame uniform",
            std::mem::size_of::<FrameUniform>() as u64,
        );
        let view_buffer = create_uniform_buffer(
            &device,
            "view uniform",
            VIEW_COUNT as u64 * VIEW_UNIFORM_STRIDE,
        );
        let frame_bind_group =
            bindings::frame_bind_group(&device, &passes.frame_layout, &frame_buffer);
        let view_bind_group = bindings::view_bind_group(&device, &passes.view_layout, &view_buffer);

        let draw_stride = gpu_scene.instance_capacity as u64 * 20;
        let draw_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("indirect draw commands"),
            size: draw_stride * VIEW_COUNT as u64,
            usage: BufferUsages::STORAGE | BufferUsages::INDIRECT,
            mapped_at_creation: false,
        });
        let count_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("visible count"),
            size: VIEW_COUNT as u64 * COUNT_STRIDE,
            usage: BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let scene_bindings = bindings::scene_bindings(
            &device,
            &passes,
            &gpu_scene,
            &targets,
            &draw_buffer,
            &count_buffer,
        );
        let target_bindings = bindings::target_bindings(&device, &queue, &passes, &targets);
        let depth_textures_bind_group = depth_textures_bind_group(&device, &passes, &gpu_scene);

        let (joint_buffer, joint_bind_group) =
            joint_resources(&device, &passes, INITIAL_JOINT_CAPACITY);

        Self {
            device,
            queue,
            config,
            output_format,
            passes,
            gpu_scene,
            gpu_driven,
            frame_buffer,
            view_buffer,
            frame_bind_group,
            view_bind_group,
            scene_bindings,
            depth_textures_bind_group,
            target_bindings,
            draw_buffer,
            count_buffer,
            joint_buffer,
            joint_bind_group,
            joint_capacity: INITIAL_JOINT_CAPACITY,
            targets,
            cascades: Cascades {
                matrices: [Mat4::IDENTITY; MAX_CASCADES],
                splits: [0.0; MAX_CASCADES],
                world_texel: 1.0,
            },
            frame_index: 0,
        }
    }

    pub fn render(&mut self, scene: &RenderScene, surface: &graphics::Surface) -> RenderOutcome {
        let (width, height) = surface.size();
        self.prepare(scene, width, height, false);

        match surface.acquire() {
            graphics::Acquisition::Ready(texture) => {
                let view = texture
                    .texture
                    .create_view(&TextureViewDescriptor::default());
                let mut encoder = self.create_encoder("frame commands");
                self.encode(scene, &mut encoder, Some(&view));
                self.queue.submit(Some(encoder.finish()));
                surface.present(&self.queue, texture);
                RenderOutcome::Presented
            }
            graphics::Acquisition::Outdated => RenderOutcome::Outdated,
            graphics::Acquisition::Lost => RenderOutcome::Lost,
            graphics::Acquisition::Timeout
            | graphics::Acquisition::Occluded
            | graphics::Acquisition::Validation => RenderOutcome::Skipped,
        }
    }

    pub fn render_offscreen(
        &mut self,
        scene: &RenderScene,
        width: u32,
        height: u32,
    ) -> RenderStats {
        self.prepare(scene, width.max(1), height.max(1), true);
        let mut encoder = self.create_encoder("offscreen commands");
        let view = self
            .targets
            .offscreen_view
            .as_ref()
            .expect("offscreen target is not available");
        self.encode(scene, &mut encoder, Some(view));
        self.queue.submit(Some(encoder.finish()));
        self.stats(scene)
    }

    pub fn read_offscreen(&mut self) -> Option<Vec<u8>> {
        let texture = self.targets.offscreen.as_ref()?;
        let (width, height) = self.targets.size;
        let padded_row = (width * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("screenshot"),
            size: (padded_row * height) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.create_encoder("screenshot commands");
        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(height),
                },
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        let slice = buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(MapMode::Read, move |result| {
            sender.send(result).ok();
        });
        self.device.poll(PollType::wait_indefinitely()).ok()?;
        receiver.recv().ok()?.ok()?;

        let data = slice.get_mapped_range().ok()?;
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height {
            let start = (row * padded_row) as usize;
            pixels.extend_from_slice(&data[start..start + (width * 4) as usize]);
        }
        drop(data);
        buffer.unmap();
        Some(pixels)
    }

    fn prepare(&mut self, scene: &RenderScene, width: u32, height: u32, offscreen: bool) {
        let targets_rebuilt = self.targets.resize(
            &self.device,
            &self.config,
            self.output_format,
            (width, height),
            offscreen,
        );
        if targets_rebuilt {
            self.target_bindings =
                bindings::target_bindings(&self.device, &self.queue, &self.passes, &self.targets);
        }

        if self.gpu_scene.revision != scene.revision {
            self.gpu_scene = GpuScene::upload(&self.device, &self.queue, &self.config, scene);
            self.scene_bindings = bindings::scene_bindings(
                &self.device,
                &self.passes,
                &self.gpu_scene,
                &self.targets,
                &self.draw_buffer,
                &self.count_buffer,
            );
            self.depth_textures_bind_group =
                depth_textures_bind_group(&self.device, &self.passes, &self.gpu_scene);
        }

        self.gpu_scene.update_instances(&self.queue, scene);
        self.gpu_scene.update_lights(&self.queue, scene);
        let joint_count = scene.joint_matrices.len();
        if joint_count > self.joint_capacity {
            let capacity = joint_count.next_power_of_two();
            let (buffer, bind_group) = joint_resources(&self.device, &self.passes, capacity);
            self.joint_buffer = buffer;
            self.joint_bind_group = bind_group;
            self.joint_capacity = capacity;
        }
        if joint_count > 0 {
            self.queue.write_buffer(
                &self.joint_buffer,
                0,
                bytemuck::cast_slice(&scene.joint_matrices[..joint_count]),
            );
        }
        self.write_uniforms(scene, width, height);
        self.frame_index += 1;
    }

    fn write_uniforms(&mut self, scene: &RenderScene, width: u32, height: u32) {
        let camera = scene.camera;
        let instance_count = scene
            .instances
            .len()
            .min(self.gpu_scene.instance_capacity as usize) as u32;
        let cascade_count = self.config.cascade_count.clamp(1, MAX_CASCADES as u32);
        let shadow_distance = scene.settings.shadow_distance.max(1.0);
        self.cascades = cascades::compute(
            &camera,
            scene.sun.direction,
            cascade_count,
            shadow_distance,
            self.config.shadow_map_size,
        );

        let mut views = [ViewUniform::new(Mat4::IDENTITY, 0, 0); VIEW_COUNT];
        views[0] = ViewUniform::new(camera.view_projection, instance_count, 0);
        views[TRANSPARENT_VIEW] = ViewUniform::new(camera.view_projection, instance_count, 1);
        for (index, matrix) in self
            .cascades
            .matrices
            .iter()
            .take(cascade_count as usize)
            .enumerate()
        {
            views[index + 1] = ViewUniform::new(*matrix, instance_count, 0);
        }
        let view_bytes: Vec<u8> = views
            .iter()
            .flat_map(|view| {
                let mut bytes = vec![0u8; VIEW_UNIFORM_SIZE];
                let raw = bytemuck::bytes_of(view);
                bytes[..raw.len()].copy_from_slice(raw);
                bytes
            })
            .collect();
        self.queue.write_buffer(&self.view_buffer, 0, &view_bytes);

        let settings = scene.settings;
        let shadow_size = self.config.shadow_map_size.max(1) as f32;
        let sun_direction = scene.sun.direction.normalize_or_zero();
        let frame = FrameUniform {
            view_proj: camera.view_projection,
            inv_view_proj: camera.view_projection.inverse(),
            camera_pos: camera.position.extend(1.0),
            sun_dir: sun_direction.extend(scene.sun.intensity),
            sun_color: scene.sun.color.extend(0.0),
            cascade_splits: self.cascades.splits.into(),
            shadow_texel: foundation::Vec4::new(
                1.0 / shadow_size,
                1.0 / shadow_size,
                0.0015,
                self.cascades.world_texel * 1.5,
            ),
            cascade_vp: self.cascades.matrices,
            exposure: foundation::Vec4::new(
                settings.exposure,
                settings.bloom,
                self.frame_index as f32,
                settings.ambient,
            ),
            params: foundation::Vec4::new(
                camera.near,
                if settings.toon { 1.0 } else { 0.0 },
                shadow_distance,
                scene
                    .lights
                    .len()
                    .min(self.gpu_scene.light_capacity as usize) as f32,
            ),
            viewport: foundation::Vec4::new(width as f32, height as f32, 0.0, 0.0),
        };
        self.queue
            .write_buffer(&self.frame_buffer, 0, bytemuck::bytes_of(&frame));
    }

    fn encode(
        &self,
        scene: &RenderScene,
        encoder: &mut CommandEncoder,
        output: Option<&TextureView>,
    ) {
        let instance_count = scene
            .instances
            .len()
            .min(self.gpu_scene.instance_capacity as usize) as u32;
        let cascade_count = self.config.cascade_count.clamp(1, MAX_CASCADES as u32);

        encoder.clear_buffer(&self.count_buffer, 0, None);

        if self.gpu_driven && instance_count > 0 {
            for view in 0..(1 + cascade_count as usize) {
                self.dispatch_cull(encoder, view, instance_count);
            }
            self.dispatch_cull(encoder, TRANSPARENT_VIEW, instance_count);
        }

        for cascade in 0..cascade_count as usize {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: self.targets.shadow_layer(cascade),
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(0.0),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.passes.shadow_pipeline);
            pass.set_bind_group(
                0,
                &self.view_bind_group,
                &[((cascade + 1) as u32) * VIEW_UNIFORM_STRIDE as u32],
            );
            pass.set_bind_group(1, &self.depth_textures_bind_group, &[]);
            pass.set_bind_group(2, &self.scene_bindings.storage, &[]);
            pass.set_bind_group(3, &self.joint_bind_group, &[]);
            self.draw_geometry(&mut pass, scene, cascade + 1, instance_count);
        }

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("depth prepass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.targets.depth_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(0.0),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.passes.depth_pipeline);
            pass.set_bind_group(0, &self.view_bind_group, &[0u32]);
            pass.set_bind_group(1, &self.depth_textures_bind_group, &[]);
            pass.set_bind_group(2, &self.scene_bindings.storage, &[]);
            pass.set_bind_group(3, &self.joint_bind_group, &[]);
            self.draw_geometry(&mut pass, scene, 0, instance_count);
        }

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("opaque"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.targets.hdr_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.targets.depth_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.passes.mesh_pipeline);
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            pass.set_bind_group(1, &self.scene_bindings.textures, &[]);
            pass.set_bind_group(2, &self.scene_bindings.storage, &[]);
            pass.set_bind_group(3, &self.joint_bind_group, &[]);
            self.draw_geometry(&mut pass, scene, 0, instance_count);
        }

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("sky"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.targets.hdr_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.targets.depth_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.passes.sky_pipeline);
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        if self.gpu_driven {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("transparent"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.targets.hdr_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.targets.depth_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.passes.blend_pipeline);
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            pass.set_bind_group(1, &self.scene_bindings.textures, &[]);
            pass.set_bind_group(2, &self.scene_bindings.storage, &[]);
            pass.set_bind_group(3, &self.joint_bind_group, &[]);
            self.draw_geometry(&mut pass, scene, TRANSPARENT_VIEW, instance_count);
        }

        for (mip, target) in self.targets.bloom.iter().enumerate() {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("bloom downsample"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &target.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(if mip == 0 {
                &self.passes.prefilter_pipeline
            } else {
                &self.passes.downsample_pipeline
            });
            pass.set_bind_group(0, &self.target_bindings.bloom_down[mip], &[]);
            pass.draw(0..3, 0..1);
        }

        for (index, bind_group) in self.target_bindings.bloom_upsample.iter().rev().enumerate() {
            let mip = self.targets.bloom.len() - 1 - index;
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("bloom upsample"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.targets.bloom[mip - 1].view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.passes.upsample_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        if let Some(view) = output {
            let tonemap = &self.target_bindings.tonemap;
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("tonemap"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.passes.tonemap_pipeline);
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            pass.set_bind_group(1, tonemap, &[]);
            pass.draw(0..3, 0..1);
        }
    }

    pub fn update_mesh_vertices(&self, scene: &RenderScene, meshes: &[u32]) {
        for &mesh_index in meshes {
            let Some(mesh) = scene.meshes.get(mesh_index as usize) else {
                continue;
            };
            let Some(range) = self.gpu_scene.mesh_ranges.get(mesh_index as usize) else {
                continue;
            };
            if mesh.vertices.len() != range.vertex_count as usize {
                log::warn!("dynamic mesh topology change is not supported");
                continue;
            }
            let offset = range.base_vertex as u64 * std::mem::size_of::<crate::Vertex>() as u64;
            self.queue.write_buffer(
                &self.gpu_scene.vertex_buffer,
                offset,
                bytemuck::cast_slice(&mesh.vertices),
            );
        }
    }

    fn dispatch_cull(&self, encoder: &mut CommandEncoder, view: usize, instance_count: u32) {
        let dispatch = instance_count.div_ceil(64);
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("culling"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.passes.cull_pipeline);
        pass.set_bind_group(
            0,
            &self.view_bind_group,
            &[(view as u32) * VIEW_UNIFORM_STRIDE as u32],
        );
        pass.set_bind_group(1, &self.scene_bindings.cull_input, &[]);
        pass.set_bind_group(2, &self.scene_bindings.cull_output[view], &[]);
        pass.dispatch_workgroups(dispatch, 1, 1);
    }

    fn draw_geometry(
        &self,
        pass: &mut RenderPass<'_>,
        scene: &RenderScene,
        view: usize,
        instance_count: u32,
    ) {
        pass.set_vertex_buffer(0, self.gpu_scene.vertex_buffer.slice(..));
        pass.set_index_buffer(self.gpu_scene.index_buffer.slice(..), IndexFormat::Uint32);

        if self.gpu_driven {
            let draw_stride = self.gpu_scene.instance_capacity as u64 * 20;
            pass.multi_draw_indexed_indirect_count(
                &self.draw_buffer,
                view as u64 * draw_stride,
                &self.count_buffer,
                view as u64 * COUNT_STRIDE,
                instance_count,
            );
            return;
        }

        for (index, instance) in scene
            .instances
            .iter()
            .take(instance_count as usize)
            .enumerate()
        {
            let Some(range) = self.gpu_scene.mesh_ranges.get(instance.mesh as usize) else {
                continue;
            };
            pass.draw_indexed(
                range.first_index..range.first_index + range.index_count,
                range.base_vertex,
                index as u32..index as u32 + 1,
            );
        }
    }

    fn stats(&self, scene: &RenderScene) -> RenderStats {
        RenderStats {
            frame_index: self.frame_index,
            instances: scene
                .instances
                .len()
                .min(self.gpu_scene.instance_capacity as usize) as u32,
            draw_calls: if self.gpu_driven {
                1
            } else {
                scene.instances.len() as u32
            },
            gpu_driven: self.gpu_driven,
        }
    }

    fn create_encoder(&self, label: &str) -> CommandEncoder {
        self.device
            .create_command_encoder(&CommandEncoderDescriptor { label: Some(label) })
    }
}

fn create_uniform_buffer(device: &Device, label: &str, size: u64) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn depth_textures_bind_group(device: &Device, passes: &Passes, scene: &GpuScene) -> BindGroup {
    device.create_bind_group(&BindGroupDescriptor {
        label: Some("depth textures bindings"),
        layout: &passes.depth_textures_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&scene.textures.albedo),
            },
            BindGroupEntry {
                binding: 4,
                resource: BindingResource::Sampler(&passes.linear_sampler),
            },
        ],
    })
}

fn joint_resources(device: &Device, passes: &Passes, capacity: usize) -> (Buffer, BindGroup) {
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("joint matrices"),
        size: capacity as u64 * std::mem::size_of::<Mat4>() as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("joint matrices bindings"),
        layout: &passes.skin_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    (buffer, bind_group)
}
