use std::collections::HashMap;
use std::path::Path;

use foundation::{Aabb, Mat4, Quat, Transform, Vec3, Vec4};
use renderer::{
    DEFAULT_TEXTURE_LAYERS, MaterialData, MeshData, TEXTURE_BLACK, TEXTURE_DEFAULT_MR,
    TEXTURE_FLAT_NORMAL, TEXTURE_WHITE, TextureData, Vertex,
};

use crate::animation::{AnimationClip, AnimationTrack, Interpolation, TrackValues};
use crate::error::AssetError;

pub struct LoadedModel {
    pub nodes: Vec<ModelNode>,
    pub roots: Vec<usize>,
    pub meshes: Vec<MeshData>,
    pub materials: Vec<MaterialData>,
    pub textures: Vec<TextureData>,
    pub animations: Vec<AnimationClip>,
}

pub struct ModelNode {
    pub name: String,
    pub parent: Option<usize>,
    pub transform: Transform,
    pub primitives: Vec<ModelPrimitive>,
}

pub struct ModelPrimitive {
    pub mesh: u32,
    pub material: u32,
}

pub fn load_gltf(path: &Path) -> Result<LoadedModel, AssetError> {
    let (document, buffers, images) = gltf::import(path)?;
    let textures = images
        .iter()
        .map(|image| {
            let pixels = convert_pixels(&image.pixels, image.format);
            TextureData::from_rgba8(image.width, image.height, pixels)
        })
        .collect();

    let texture_layers: HashMap<usize, usize> = document
        .textures()
        .filter_map(|texture| {
            document
                .images()
                .position(|image| image.index() == texture.source().index())
                .map(|layer| (texture.index(), layer))
        })
        .collect();

    let materials = document
        .materials()
        .map(|material| convert_material(&material, &texture_layers))
        .collect();

    let mut nodes: Vec<ModelNode> = document
        .nodes()
        .map(|node| {
            let (scale, rotation, translation) =
                Mat4::from_cols_array_2d(&node.transform().matrix())
                    .to_scale_rotation_translation();
            ModelNode {
                name: node.name().unwrap_or("node").to_string(),
                parent: None,
                transform: Transform {
                    translation,
                    rotation,
                    scale,
                },
                primitives: Vec::new(),
            }
        })
        .collect();

    let mut meshes = Vec::new();
    for node in document.nodes() {
        let Some(mesh) = node.mesh() else {
            continue;
        };
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                log::warn!("skipping non-triangle primitive: {:?}", primitive.mode());
                continue;
            }
            let Some(mesh_data) = read_primitive(&primitive, &buffers) else {
                continue;
            };
            nodes[node.index()].primitives.push(ModelPrimitive {
                mesh: meshes.len() as u32,
                material: primitive.material().index().unwrap_or(0) as u32,
            });
            meshes.push(mesh_data);
        }
    }

    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next());
    let scene_nodes: Vec<gltf::Node<'_>> = scene
        .as_ref()
        .map(|scene| scene.nodes().collect())
        .unwrap_or_default();
    let roots: Vec<usize> = scene_nodes.iter().map(|node| node.index()).collect();
    for root in &scene_nodes {
        assign_parents(root, &mut nodes);
    }

    let animations = load_animations(&document, &buffers);

    Ok(LoadedModel {
        nodes,
        roots,
        meshes,
        materials,
        textures,
        animations,
    })
}

fn normalize_quat(value: [f32; 4]) -> Quat {
    let rotation = Quat::from_array(value);
    if rotation.length_squared() > 1e-8 {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    }
}

fn assign_parents(node: &gltf::Node<'_>, nodes: &mut [ModelNode]) {
    for child in node.children() {
        nodes[child.index()].parent = Some(node.index());
        assign_parents(&child, nodes);
    }
}

fn load_animations(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
) -> Vec<AnimationClip> {
    document
        .animations()
        .map(|animation| {
            let mut tracks = Vec::new();
            for channel in animation.channels() {
                let interpolation = match channel.sampler().interpolation() {
                    gltf::animation::Interpolation::Linear => Interpolation::Linear,
                    gltf::animation::Interpolation::Step => Interpolation::Step,
                    gltf::animation::Interpolation::CubicSpline => {
                        log::warn!("skipping cubic spline animation channel");
                        continue;
                    }
                };
                let reader = channel.reader(|buffer| Some(&buffers[buffer.index()].0));
                let Some(inputs) = reader.read_inputs() else {
                    continue;
                };
                let times: Vec<f32> = inputs.collect();
                let Some(outputs) = reader.read_outputs() else {
                    continue;
                };
                let values = match outputs {
                    gltf::animation::util::ReadOutputs::Translations(samples) => {
                        TrackValues::Translations(samples.map(Vec3::from_array).collect())
                    }
                    gltf::animation::util::ReadOutputs::Rotations(samples) => {
                        TrackValues::Rotations(samples.into_f32().map(normalize_quat).collect())
                    }
                    gltf::animation::util::ReadOutputs::Scales(samples) => {
                        TrackValues::Scales(samples.map(Vec3::from_array).collect())
                    }
                    gltf::animation::util::ReadOutputs::MorphTargetWeights(_) => continue,
                };
                let sample_count = match &values {
                    TrackValues::Translations(samples) => samples.len(),
                    TrackValues::Rotations(samples) => samples.len(),
                    TrackValues::Scales(samples) => samples.len(),
                };
                if times.len() < 2 || times.len() != sample_count {
                    continue;
                }
                tracks.push(AnimationTrack {
                    node: channel.target().node().index(),
                    interpolation,
                    times,
                    values,
                });
            }
            let duration = tracks
                .iter()
                .filter_map(|track| track.times.last().copied())
                .fold(0.0f32, f32::max);
            AnimationClip {
                name: animation.name().unwrap_or("animation").to_string(),
                duration,
                tracks,
            }
        })
        .collect()
}

fn read_primitive(
    primitive: &gltf::Primitive<'_>,
    buffers: &[gltf::buffer::Data],
) -> Option<MeshData> {
    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()].0));
    let positions: Vec<[f32; 3]> = reader.read_positions()?.collect();
    let indices: Vec<u32> = reader.read_indices()?.into_u32().collect();
    let uvs: Vec<[f32; 2]> = reader
        .read_tex_coords(0)
        .map(|coords| coords.into_f32().collect())
        .unwrap_or_default();
    let normals = reader
        .read_normals()
        .map(|normals| normals.collect())
        .unwrap_or_else(|| generate_normals(&positions, &indices));
    let tangents = reader
        .read_tangents()
        .map(|tangents| tangents.collect())
        .unwrap_or_else(|| generate_tangents(&positions, &normals, &uvs, &indices));

    let vertices = positions
        .iter()
        .enumerate()
        .map(|(index, position)| Vertex {
            position: *position,
            normal: normals.get(index).copied().unwrap_or([0.0, 1.0, 0.0]),
            uv: uvs.get(index).copied().unwrap_or([0.0, 0.0]),
            tangent: tangents.get(index).copied().unwrap_or([1.0, 0.0, 0.0, 1.0]),
        })
        .collect();

    Some(MeshData {
        vertices,
        indices,
        bounds: Aabb::from_points(positions.iter().map(|position| Vec3::from_array(*position))),
    })
}

fn convert_material(
    material: &gltf::Material<'_>,
    texture_layers: &HashMap<usize, usize>,
) -> MaterialData {
    let pbr = material.pbr_metallic_roughness();
    let base_color = pbr.base_color_factor();
    let emissive = material.emissive_factor();
    let layer = |texture: Option<gltf::Texture<'_>>| {
        texture
            .and_then(|texture| texture_layers.get(&texture.index()).copied())
            .map(|layer| DEFAULT_TEXTURE_LAYERS + layer as u32)
    };

    MaterialData {
        base_color: Vec4::from_array(base_color),
        emissive_metallic: Vec4::new(emissive[0], emissive[1], emissive[2], pbr.metallic_factor()),
        roughness_uv_scale: Vec4::new(pbr.roughness_factor(), 1.0, 1.0, 0.0),
        uv_offset_flags: Vec4::ZERO,
        textures: [
            layer(pbr.base_color_texture().map(|info| info.texture())).unwrap_or(TEXTURE_WHITE),
            layer(material.normal_texture().map(|info| info.texture()))
                .unwrap_or(TEXTURE_FLAT_NORMAL),
            layer(pbr.metallic_roughness_texture().map(|info| info.texture()))
                .unwrap_or(TEXTURE_DEFAULT_MR),
            layer(material.emissive_texture().map(|info| info.texture())).unwrap_or(TEXTURE_BLACK),
        ],
    }
}

fn convert_pixels(pixels: &[u8], format: gltf::image::Format) -> Vec<u8> {
    use gltf::image::Format;

    match format {
        Format::R8 => pixels.iter().flat_map(|&r| [r, r, r, 255]).collect(),
        Format::R8G8 => pixels
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|chunk| [chunk[0], chunk[1], 0, 255])
            .collect(),
        Format::R8G8B8 => pixels
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|chunk| [chunk[0], chunk[1], chunk[2], 255])
            .collect(),
        Format::R8G8B8A8 => pixels.to_vec(),
        _ => pixels
            .as_chunks::<8>()
            .0
            .iter()
            .flat_map(|chunk| [chunk[0], chunk[2], chunk[4], 255])
            .collect(),
    }
}

fn generate_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![Vec3::ZERO; positions.len()];
    for triangle in indices.as_chunks::<3>().0.iter() {
        let a = Vec3::from_array(positions[triangle[0] as usize]);
        let b = Vec3::from_array(positions[triangle[1] as usize]);
        let c = Vec3::from_array(positions[triangle[2] as usize]);
        let normal = (b - a).cross(c - a);
        for &index in triangle {
            normals[index as usize] += normal;
        }
    }
    normals
        .into_iter()
        .map(|normal| normal.normalize_or_zero().to_array())
        .collect()
}

fn generate_tangents(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> Vec<[f32; 4]> {
    let mut tangents = vec![Vec3::ZERO; positions.len()];
    let mut bitangents = vec![Vec3::ZERO; positions.len()];

    for triangle in indices.as_chunks::<3>().0.iter() {
        let [i0, i1, i2] = [
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        ];
        let p0 = Vec3::from_array(positions[i0]);
        let p1 = Vec3::from_array(positions[i1]);
        let p2 = Vec3::from_array(positions[i2]);
        let uv0 = uvs.get(i0).copied().unwrap_or([0.0, 0.0]);
        let uv1 = uvs.get(i1).copied().unwrap_or([0.0, 0.0]);
        let uv2 = uvs.get(i2).copied().unwrap_or([0.0, 0.0]);
        let edge1 = p1 - p0;
        let edge2 = p2 - p0;
        let du1 = uv1[0] - uv0[0];
        let dv1 = uv1[1] - uv0[1];
        let du2 = uv2[0] - uv0[0];
        let dv2 = uv2[1] - uv0[1];
        let determinant = du1 * dv2 - du2 * dv1;
        if determinant.abs() < 1e-8 {
            continue;
        }
        let inverse = 1.0 / determinant;
        let tangent = (edge1 * dv2 - edge2 * dv1) * inverse;
        let bitangent = (edge2 * du1 - edge1 * du2) * inverse;
        for index in [i0, i1, i2] {
            tangents[index] += tangent;
            bitangents[index] += bitangent;
        }
    }

    tangents
        .iter()
        .enumerate()
        .map(|(index, tangent)| {
            let normal = Vec3::from_array(normals.get(index).copied().unwrap_or([0.0, 1.0, 0.0]));
            let tangent = tangent.normalize_or_zero();
            let handedness = if normal.cross(tangent).dot(bitangents[index]) < 0.0 {
                -1.0
            } else {
                1.0
            };
            [tangent.x, tangent.y, tangent.z, handedness]
        })
        .collect()
}
