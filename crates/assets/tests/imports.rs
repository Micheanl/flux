use std::path::{Path, PathBuf};

use assets::{load_gltf, load_image};

fn fixture(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures");
    path.push(name);
    path
}

fn load(name: &str) -> assets::LoadedModel {
    let path = fixture(name);
    load_gltf(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn loads_minimal_gltf() {
    let model = load("minimal.gltf");
    assert_eq!(model.meshes.len(), 1);
    assert_eq!(model.meshes[0].vertices.len(), 3);
    assert_eq!(model.meshes[0].indices, vec![0, 1, 2]);
    assert!(model.meshes[0].bounds.is_valid());
    assert_eq!(model.nodes.len(), 1);
    assert_eq!(model.nodes[0].primitives.len(), 1);
    assert_eq!(model.roots, vec![0]);
    assert!((model.nodes[0].transform.translation.y - 2.0).abs() < 1e-5);
    assert_eq!(model.materials.len(), 1);
    assert!((model.materials[0].base_color.x - 1.0).abs() < 1e-5);
    assert!((model.materials[0].base_color.y - 0.2).abs() < 1e-5);
    assert!((model.materials[0].emissive_metallic.w - 1.0).abs() < 1e-5);
}

#[test]
fn loads_png_image() {
    let texture = load_image(&fixture("image.png")).expect("image loading failed");
    assert_eq!(texture.width, 2);
    assert_eq!(texture.height, 2);
    assert_eq!(texture.rgba.len(), 16);
    assert_eq!(&texture.rgba[0..4], &[255, 0, 0, 255]);
}

#[test]
fn loads_animation_tracks() {
    let model = load("animated.gltf");
    assert_eq!(model.animations.len(), 1);
    let clip = &model.animations[0];
    assert_eq!(clip.name, "bob");
    assert!((clip.duration - 1.0).abs() < 1e-5);
    assert_eq!(clip.tracks.len(), 1);
    let poses = clip.sample(0.5);
    assert_eq!(poses.len(), 1);
    let translation = poses[0].translation.expect("missing translation");
    assert!((translation - foundation::Vec3::new(0.0, 1.5, 0.0)).length() < 1e-5);
}

#[test]
fn loads_skinned_primitive() {
    let model = load("skinned.gltf");
    assert_eq!(model.skins.len(), 1);
    assert_eq!(model.skins[0].joints, vec![0]);
    assert_eq!(model.skins[0].inverse_bind_matrices.len(), 1);
    assert_eq!(model.nodes[1].primitives.len(), 1);
    assert_eq!(model.nodes[1].primitives[0].skin, Some(0));
    assert_eq!(model.meshes[0].vertices[0].joints, [0; 4]);
    assert_eq!(model.meshes[0].vertices[0].weights, [1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn loads_material_extensions() {
    let model = load("materials.gltf");
    let material = model.materials[0];
    assert!((material.flags.x - 1.0).abs() < 1e-5);
    assert!((material.flags.y - 1.0).abs() < 1e-5);
    assert!((material.flags.z - 1.0).abs() < 1e-5);
    assert!((material.flags.w - 0.3).abs() < 1e-5);
    assert!((material.emissive_metallic.x - 2.0).abs() < 1e-5);
    assert!((material.emissive_metallic.y - 1.0).abs() < 1e-5);
    assert!((material.emissive_metallic.z - 0.5).abs() < 1e-5);
}

#[test]
fn loads_morph_targets_and_weights() {
    let model = load("morph.gltf");
    let morph = model.nodes[0].primitives[0]
        .morph
        .as_ref()
        .expect("morph targets");
    assert_eq!(morph.targets.len(), 1);
    assert_eq!(morph.targets[0].positions.len(), 3);
    assert_eq!(morph.weights, vec![0.5]);
    assert_eq!(model.animations.len(), 1);
    let pose = model.animations[0].sample(0.5);
    let weights = pose[0].weights.as_ref().expect("sampled weights");
    assert!((weights[0] - 0.5).abs() < 1e-5);
}

#[test]
fn loads_punctual_lights() {
    let model = load("lights.gltf");
    assert_eq!(model.lights.len(), 2);
    for light in &model.lights {
        assert!(matches!(light.kind, assets::LightKind::Point { .. }));
    }
    let assets::LightKind::Point {
        color,
        intensity,
        range,
    } = model.lights[0].kind
    else {
        unreachable!()
    };
    assert!((intensity - 20.0).abs() < 1e-5);
    assert!((range - 14.0).abs() < 1e-5);
    assert!((color.x - 1.0).abs() < 1e-5);
}

#[test]
fn loads_spot_lights() {
    let model = load("spotlights.gltf");
    assert_eq!(model.lights.len(), 3);
    for light in &model.lights {
        let assets::LightKind::Spot {
            inner_cone,
            outer_cone,
            intensity,
            ..
        } = light.kind
        else {
            panic!("expected spot light");
        };
        assert!(inner_cone < outer_cone);
        assert!(intensity > 1.0);
    }
}

#[test]
fn loads_mmd_motion() {
    let clip = assets::load_mmd_animation(&fixture("motion_cube.pmx"), &fixture("motion_cube.vmd"))
        .expect("MMD motion loading failed");
    assert!(clip.duration > 0.0);
    assert!(!clip.tracks.is_empty());
    let pose = clip.sample(clip.duration * 0.5);
    assert!(pose.iter().any(|pose| pose.rotation.is_some()));
}

#[test]
fn loads_sandbox_demo_models() {
    let directory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sandbox/assets/models");
    let mut models: Vec<PathBuf> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "gltf")
        })
        .collect();
    models.sort();
    assert_eq!(models.len(), 5);
    for path in models {
        let model = load_gltf(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(!model.meshes.is_empty(), "{}", path.display());
    }
}

#[test]
fn loads_mmd_model() {
    let model = assets::load_mmd(&fixture("robot.pmx"), Some(&fixture("robot.vmd")))
        .expect("MMD loading failed");
    assert_eq!(model.meshes.len(), 2);
    assert_eq!(model.materials.len(), 2);
    assert_eq!(model.textures.len(), 2);
    assert_eq!(model.skins.len(), 1);
    assert_eq!(model.skins[0].joints.len(), 7);
    assert_eq!(model.animations.len(), 1);
    let clip = &model.animations[0];
    assert!(clip.duration > 1.9);
    assert!(clip.tracks.len() >= 5);
    let pose = clip.sample(0.5);
    let arm = pose.iter().find(|pose| pose.node == 3).expect("arm track");
    assert!(arm.rotation.is_some());
    assert!(model.roots.len() >= 2);
}
