use bytemuck::cast_slice;
use foundation::Vec4;
use wgpu::{Buffer, BufferDescriptor, BufferUsages, Device, Queue};

use crate::config::RendererConfig;
use crate::mesh::MaterialData;
use crate::scene::{PointLight, RenderScene};
use crate::textures::{TextureArrays, upload_textures};

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuMesh {
    index_offset: u32,
    index_count: u32,
    base_vertex: i32,
    _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuPointLight {
    position: Vec4,
    color: Vec4,
}

impl From<&PointLight> for GpuPointLight {
    fn from(light: &PointLight) -> Self {
        Self {
            position: light.position.extend(light.radius),
            color: (light.color * light.intensity).extend(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MeshRange {
    pub first_index: u32,
    pub index_count: u32,
    pub base_vertex: i32,
    pub vertex_count: u32,
}

pub struct GpuScene {
    pub revision: u64,
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub mesh_buffer: Buffer,
    pub material_buffer: Buffer,
    pub instance_buffer: Buffer,
    pub light_buffer: Buffer,
    pub textures: TextureArrays,
    pub mesh_ranges: Vec<MeshRange>,
    pub instance_capacity: u32,
    pub light_capacity: u32,
}

impl GpuScene {
    pub fn upload(
        device: &Device,
        queue: &Queue,
        config: &RendererConfig,
        scene: &RenderScene,
    ) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut meshes = Vec::with_capacity(scene.meshes.len());
        let mut mesh_ranges = Vec::with_capacity(scene.meshes.len());

        for mesh in &scene.meshes {
            mesh_ranges.push(MeshRange {
                first_index: indices.len() as u32,
                index_count: mesh.indices.len() as u32,
                base_vertex: vertices.len() as i32,
                vertex_count: mesh.vertices.len() as u32,
            });
            meshes.push(GpuMesh {
                index_offset: indices.len() as u32,
                index_count: mesh.indices.len() as u32,
                base_vertex: vertices.len() as i32,
                _pad: 0,
            });
            vertices.extend_from_slice(&mesh.vertices);
            indices.extend_from_slice(&mesh.indices);
        }

        let vertex_buffer = create_buffer(
            device,
            "vertices",
            std::mem::size_of_val(vertices.as_slice()) as u64,
            BufferUsages::VERTEX | BufferUsages::COPY_DST,
        );
        if !vertices.is_empty() {
            queue.write_buffer(&vertex_buffer, 0, cast_slice(&vertices));
        }

        let index_buffer = create_buffer(
            device,
            "indices",
            std::mem::size_of_val(indices.as_slice()) as u64,
            BufferUsages::INDEX | BufferUsages::COPY_DST,
        );
        if !indices.is_empty() {
            queue.write_buffer(&index_buffer, 0, cast_slice(&indices));
        }

        let mesh_buffer = create_buffer(
            device,
            "mesh metadata",
            (meshes.len().max(1) * std::mem::size_of::<GpuMesh>()) as u64,
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
        );
        if !meshes.is_empty() {
            queue.write_buffer(&mesh_buffer, 0, cast_slice(&meshes));
        }

        let material_buffer = create_buffer(
            device,
            "materials",
            (scene.materials.len().max(1) * std::mem::size_of::<MaterialData>()) as u64,
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
        );
        if !scene.materials.is_empty() {
            queue.write_buffer(&material_buffer, 0, cast_slice(&scene.materials));
        }

        let instance_capacity = config.max_instances.max(1);
        let instance_buffer = create_buffer(
            device,
            "instances",
            instance_capacity as u64 * std::mem::size_of::<crate::scene::InstanceData>() as u64,
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
        );

        let light_capacity = 256u32;
        let light_buffer = create_buffer(
            device,
            "point lights",
            light_capacity as u64 * std::mem::size_of::<GpuPointLight>() as u64,
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
        );

        let textures = upload_textures(device, queue, config, &scene.textures);

        Self {
            revision: scene.revision,
            vertex_buffer,
            index_buffer,
            mesh_buffer,
            material_buffer,
            instance_buffer,
            light_buffer,
            textures,
            mesh_ranges,
            instance_capacity,
            light_capacity,
        }
    }

    pub fn update_instances(&self, queue: &Queue, scene: &RenderScene) {
        let count = scene.instances.len().min(self.instance_capacity as usize);
        if count > 0 {
            queue.write_buffer(
                &self.instance_buffer,
                0,
                cast_slice(&scene.instances[..count]),
            );
        }
    }

    pub fn update_lights(&self, queue: &Queue, scene: &RenderScene) {
        let count = scene.point_lights.len().min(self.light_capacity as usize);
        if count > 0 {
            let lights: Vec<GpuPointLight> = scene.point_lights[..count]
                .iter()
                .map(GpuPointLight::from)
                .collect();
            queue.write_buffer(&self.light_buffer, 0, cast_slice(&lights));
        }
    }
}

fn create_buffer(device: &Device, label: &str, size: u64, usage: BufferUsages) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: size.max(16),
        usage,
        mapped_at_creation: false,
    })
}
