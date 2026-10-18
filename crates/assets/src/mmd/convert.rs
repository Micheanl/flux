use std::collections::HashMap;
use std::path::Path;

use foundation::{Aabb, Mat4, Quat, Transform, Vec3, Vec4};
use renderer::{
    AlphaMode, DEFAULT_TEXTURE_LAYERS, MaterialData, MeshData, TEXTURE_BLACK, TEXTURE_DEFAULT_MR,
    TEXTURE_FLAT_NORMAL, TEXTURE_WHITE, TextureData, Vertex,
};

use crate::animation::{AnimationClip, AnimationTrack, Interpolation, TrackValues};
use crate::error::AssetError;
use crate::gltf_loader::{LoadedModel, ModelNode, ModelPrimitive, ModelSkin};
use crate::image_loader::load_image;
use crate::mmd::pmx::{PmxMaterial, PmxModel, load_pmx};
use crate::mmd::vmd::{VmdMotion, load_vmd};

const MODEL_SCALE: f32 = 0.08;
const FRAME_RATE: f32 = 30.0;

pub fn load_mmd(path: &Path, motion: Option<&Path>) -> Result<LoadedModel, AssetError> {
    let model = load_pmx(path)?;
    let directory = path.parent().unwrap_or(Path::new("."));
    let motion = match motion {
        Some(motion) => Some(load_vmd(motion)?),
        None => {
            let sibling = path.with_extension("vmd");
            if sibling.exists() {
                Some(load_vmd(&sibling)?)
            } else {
                None
            }
        }
    };
    Ok(convert(model, motion.as_ref(), directory))
}

fn convert(model: PmxModel, motion: Option<&VmdMotion>, directory: &Path) -> LoadedModel {
    let mut textures = Vec::new();
    let mut texture_layers: HashMap<i32, u32> = HashMap::new();
    for (index, _) in model.textures.iter().enumerate() {
        let material_uses = model
            .materials
            .iter()
            .any(|material| material.texture == index as i32);
        if !material_uses {
            continue;
        }
        let path = resolve_texture(directory, &model.textures[index]);
        let texture = path
            .as_deref()
            .and_then(|path| load_image(path).ok())
            .unwrap_or_else(|| TextureData::from_rgba8(1, 1, vec![255, 255, 255, 255]));
        texture_layers.insert(index as i32, DEFAULT_TEXTURE_LAYERS + textures.len() as u32);
        textures.push(texture);
    }

    let rest_positions: Vec<Vec3> = model
        .bones
        .iter()
        .map(|bone| convert_position(bone.position))
        .collect();
    let inverse_bind_matrices: Vec<Mat4> = rest_positions
        .iter()
        .map(|position| Mat4::from_translation(-*position))
        .collect();

    let mut nodes: Vec<ModelNode> = model
        .bones
        .iter()
        .enumerate()
        .map(|(index, bone)| {
            let parent_position = bone
                .parent
                .try_into()
                .ok()
                .and_then(|parent: usize| rest_positions.get(parent).copied())
                .unwrap_or(Vec3::ZERO);
            ModelNode {
                name: bone.name.clone(),
                parent: bone.parent.try_into().ok(),
                transform: Transform::from_translation(rest_positions[index] - parent_position),
                primitives: Vec::new(),
            }
        })
        .collect();

    let mut meshes = Vec::new();
    let mut materials = Vec::new();
    let mut index_offset = 0usize;
    let mesh_node = nodes.len();
    let mut primitives = Vec::new();
    for material in &model.materials {
        let count = material.surface_count as usize;
        let end = (index_offset + count).min(model.indices.len());
        let source = &model.indices[index_offset.min(model.indices.len())..end];
        index_offset = end;
        if source.is_empty() {
            continue;
        }
        let mut indices = Vec::with_capacity(source.len());
        for triangle in source.as_chunks::<3>().0 {
            indices.extend_from_slice(&[triangle[0], triangle[2], triangle[1]]);
        }
        let mesh = MeshData {
            vertices: model
                .vertices
                .iter()
                .map(|vertex| Vertex {
                    position: convert_position(vertex.position).to_array(),
                    normal: convert_normal(vertex.normal).to_array(),
                    uv: vertex.uv.to_array(),
                    tangent: [1.0, 0.0, 0.0, 1.0],
                    joints: vertex.joints,
                    weights: vertex.weights,
                })
                .collect(),
            indices,
            bounds: Aabb::from_points(
                model
                    .vertices
                    .iter()
                    .map(|vertex| convert_position(vertex.position)),
            ),
        };
        primitives.push(ModelPrimitive {
            mesh: meshes.len() as u32,
            material: materials.len() as u32,
            skin: Some(0),
            morph: None,
        });
        meshes.push(mesh);
        materials.push(convert_material(material, &texture_layers));
    }

    nodes.push(ModelNode {
        name: model.name.clone(),
        parent: None,
        transform: Transform::default(),
        primitives,
    });

    let mut roots: Vec<usize> = model
        .bones
        .iter()
        .enumerate()
        .filter(|(_, bone)| bone.parent < 0)
        .map(|(index, _)| index)
        .collect();
    roots.push(mesh_node);

    let animations = match motion {
        Some(motion) => convert_motion(motion, &model),
        None => Vec::new(),
    };

    LoadedModel {
        nodes,
        roots,
        meshes,
        materials,
        textures,
        skins: vec![ModelSkin {
            joints: (0..model.bones.len()).collect(),
            inverse_bind_matrices,
        }],
        lights: Vec::new(),
        animations,
    }
}

fn resolve_texture(directory: &Path, name: &str) -> Option<std::path::PathBuf> {
    let cleaned = name.replace('\\', "/");
    let path = directory.join(&cleaned);
    if path.exists() {
        return Some(path);
    }
    let file = Path::new(&cleaned)
        .file_name()?
        .to_string_lossy()
        .to_lowercase();
    std::fs::read_dir(directory)
        .ok()?
        .flatten()
        .find_map(|entry| {
            let candidate = entry.path();
            let name = candidate.file_name()?.to_string_lossy().to_lowercase();
            (name == file).then_some(candidate)
        })
}

fn convert_material(material: &PmxMaterial, texture_layers: &HashMap<i32, u32>) -> MaterialData {
    let albedo = texture_layers
        .get(&material.texture)
        .copied()
        .unwrap_or(TEXTURE_WHITE);
    let data = MaterialData {
        base_color: material.diffuse,
        emissive_metallic: Vec4::new(0.0, 0.0, 0.0, 0.0),
        roughness_uv_scale: Vec4::new(0.75, 1.0, 1.0, 0.0),
        uv_offset_flags: Vec4::ZERO,
        textures: [
            albedo,
            TEXTURE_FLAT_NORMAL,
            TEXTURE_DEFAULT_MR,
            TEXTURE_BLACK,
        ],
        flags: Vec4::ZERO,
    };
    let data = data.with_double_sided(material.double_sided);
    if material.diffuse.w < 0.999 {
        data.with_alpha_mode(AlphaMode::Blend, 0.5)
    } else {
        data
    }
}

fn convert_motion(motion: &VmdMotion, model: &PmxModel) -> Vec<AnimationClip> {
    let mut tracks = Vec::new();
    for track in &motion.bones {
        let Some(node) = model.bones.iter().position(|bone| bone.name == track.name) else {
            continue;
        };
        if track.frames.is_empty() {
            continue;
        }
        let times: Vec<f32> = track
            .frames
            .iter()
            .map(|frame| frame.frame as f32 / FRAME_RATE)
            .collect();
        let translations: Vec<Vec3> = track
            .frames
            .iter()
            .map(|frame| {
                let rest = model
                    .bones
                    .get(node)
                    .map(|bone| {
                        let parent = bone
                            .parent
                            .try_into()
                            .ok()
                            .and_then(|parent: usize| model.bones.get(parent))
                            .map(|parent| parent.position)
                            .unwrap_or(Vec3::ZERO);
                        convert_position(bone.position) - convert_position(parent)
                    })
                    .unwrap_or(Vec3::ZERO);
                rest + convert_position(frame.translation)
            })
            .collect();
        if translations
            .iter()
            .any(|value| value.length_squared() > 0.0)
        {
            tracks.push(AnimationTrack {
                node,
                interpolation: Interpolation::Linear,
                times: times.clone(),
                values: TrackValues::Translations(translations),
            });
        }
        let rotations: Vec<Quat> = track
            .frames
            .iter()
            .map(|frame| convert_rotation(frame.rotation))
            .collect();
        tracks.push(AnimationTrack {
            node,
            interpolation: Interpolation::Linear,
            times,
            values: TrackValues::Rotations(rotations),
        });
    }
    if tracks.is_empty() {
        return Vec::new();
    }
    let duration = tracks
        .iter()
        .filter_map(|track| track.times.last().copied())
        .fold(0.0f32, f32::max);
    vec![AnimationClip {
        name: "motion".to_string(),
        duration,
        tracks,
    }]
}

fn convert_position(value: Vec3) -> Vec3 {
    Vec3::new(value.x, value.y, -value.z) * MODEL_SCALE
}

fn convert_normal(value: Vec3) -> Vec3 {
    Vec3::new(value.x, value.y, -value.z).normalize_or_zero()
}

fn convert_rotation(value: Quat) -> Quat {
    let rotation = Quat::from_xyzw(-value.x, -value.y, value.z, value.w);
    if rotation.length_squared() > 1e-8 {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    }
}
