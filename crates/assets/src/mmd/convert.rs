use std::collections::HashMap;
use std::path::Path;

use foundation::{Aabb, Mat4, Quat, Transform, Vec3, Vec4};
use renderer::{
    AlphaMode, DEFAULT_TEXTURE_LAYERS, MaterialData, MeshData, TEXTURE_BLACK, TEXTURE_DEFAULT_MR,
    TEXTURE_FLAT_NORMAL, TEXTURE_WHITE, TextureData, Vertex,
};

use crate::animation::{AnimationClip, AnimationTrack, Interpolation, TrackValues};
use crate::error::AssetError;
use crate::gltf_loader::{LoadedModel, ModelInherit, ModelNode, ModelPrimitive, ModelSkin};
use crate::mmd::pmx::{PmxMaterial, PmxModel, PmxVertex, load_pmx};
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
    let mut transparent_textures = std::collections::HashSet::new();
    for (index, _) in model.textures.iter().enumerate() {
        let material_uses = model
            .materials
            .iter()
            .any(|material| material.texture == index as i32);
        if !material_uses {
            continue;
        }
        let path = resolve_texture(directory, &model.textures[index]);
        let (texture, has_alpha) = match path.as_deref() {
            Some(path) => match load_image_with_alpha(path) {
                Ok(texture) => texture,
                Err(error) => {
                    log::debug!("missing texture {:?}: {error}", model.textures[index]);
                    (
                        TextureData::from_rgba8(1, 1, vec![255, 255, 255, 255]),
                        false,
                    )
                }
            },
            None => (
                TextureData::from_rgba8(1, 1, vec![255, 255, 255, 255]),
                false,
            ),
        };
        if has_alpha {
            transparent_textures.insert(index as i32);
        }
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
                inherit: bone.inherit.and_then(|inherit| {
                    Some(ModelInherit {
                        source: inherit.source.try_into().ok()?,
                        ratio: inherit.ratio,
                        rotation: inherit.rotation,
                        translation: inherit.translation,
                    })
                }),
                primitives: Vec::new(),
            }
        })
        .collect();

    let mut meshes = Vec::new();
    let mut materials = Vec::new();
    let mut index_offset = 0usize;
    let total_surfaces: u32 = model
        .materials
        .iter()
        .map(|material| material.surface_count)
        .sum();
    log::debug!(
        "mmd materials: {} entries, {} surfaces, {} indices",
        model.materials.len(),
        total_surfaces,
        model.indices.len()
    );
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
        let mut remap: HashMap<u32, u32> = HashMap::new();
        let mut source_vertices = Vec::new();
        let mut vertices = Vec::new();
        let mut indices = Vec::with_capacity(source.len());
        for triangle in source.as_chunks::<3>().0 {
            for index in [triangle[0], triangle[2], triangle[1]] {
                let remapped = match remap.get(&index) {
                    Some(remapped) => *remapped,
                    None => {
                        let remapped = vertices.len() as u32;
                        remap.insert(index, remapped);
                        let vertex =
                            model
                                .vertices
                                .get(index as usize)
                                .cloned()
                                .unwrap_or(PmxVertex {
                                    position: Vec3::ZERO,
                                    normal: Vec3::Y,
                                    uv: foundation::Vec2::ZERO,
                                    joints: [0; 4],
                                    weights: [0.0; 4],
                                });
                        source_vertices.push(convert_position(vertex.position));
                        vertices.push(Vertex {
                            position: convert_position(vertex.position).to_array(),
                            normal: convert_normal(vertex.normal).to_array(),
                            uv: vertex.uv.to_array(),
                            tangent: [1.0, 0.0, 0.0, 1.0],
                            joints: vertex.joints,
                            weights: vertex.weights,
                        });
                        remapped
                    }
                };
                indices.push(remapped);
            }
        }
        let mesh = MeshData {
            vertices,
            indices,
            bounds: Aabb::from_points(source_vertices),
        };
        primitives.push(ModelPrimitive {
            mesh: meshes.len() as u32,
            material: materials.len() as u32,
            skin: Some(0),
            morph: None,
        });
        meshes.push(mesh);
        materials.push(convert_material(
            material,
            &texture_layers,
            transparent_textures.contains(&material.texture),
        ));
    }

    nodes.push(ModelNode {
        name: model.name.clone(),
        parent: None,
        transform: Transform::default(),
        inherit: None,
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

fn convert_material(
    material: &PmxMaterial,
    texture_layers: &HashMap<i32, u32>,
    transparent: bool,
) -> MaterialData {
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
    if transparent || material.diffuse.w < 0.999 {
        data.with_alpha_mode(AlphaMode::Blend, 0.5)
    } else {
        data
    }
}

fn load_image_with_alpha(path: &Path) -> Result<(TextureData, bool), AssetError> {
    let mut image = image::open(path)?.to_rgba8();
    let alpha_path = path.with_file_name(format!(
        "{}_Alpha.png",
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
    ));
    let allow_alpha = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.starts_with("Bangs")
                || name.starts_with("Hair")
                || name.starts_with("Tail")
                || name.starts_with("Fur")
        });
    if allow_alpha && alpha_path.exists() {
        let mask = image::open(&alpha_path)?.to_luma8();
        if mask.dimensions() == image.dimensions() {
            for (pixel, mask) in image.pixels_mut().zip(mask.pixels()) {
                pixel.0[3] = ((pixel.0[3] as u16 * mask.0[0] as u16) / 255) as u8;
            }
        }
    }
    let (width, height) = image.dimensions();
    let has_alpha = image.pixels().any(|pixel| pixel.0[3] < 250);
    Ok((
        TextureData::from_rgba8(width, height, image.into_raw()),
        has_alpha,
    ))
}

pub fn load_mmd_animation(
    model_path: &Path,
    motion_path: &Path,
) -> Result<AnimationClip, AssetError> {
    let model = load_pmx(model_path)?;
    let motion = load_vmd(motion_path)?;
    convert_motion(&motion, &model)
        .pop()
        .ok_or_else(|| AssetError::Unsupported("motion does not match model bones".to_string()))
}

fn convert_motion(motion: &VmdMotion, model: &PmxModel) -> Vec<AnimationClip> {
    let mut tracks = Vec::new();
    let mut unmatched = Vec::new();
    for track in &motion.bones {
        let Some(node) = model.bones.iter().position(|bone| bone.name == track.name) else {
            unmatched.push(track.name.as_str());
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
    log::debug!(
        "mmd motion: {} tracks matched, {} unmatched: {:?}",
        tracks.len() / 2,
        unmatched.len(),
        &unmatched[..unmatched.len().min(12)]
    );
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
