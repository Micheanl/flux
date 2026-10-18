use std::path::PathBuf;

use assets::load_gltf;

#[test]
fn loads_official_samples() {
    let Ok(directory) = std::env::var("FLUX_SAMPLE_DIR") else {
        return;
    };
    let mut models: Vec<PathBuf> = std::fs::read_dir(&directory)
        .expect("sample directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "glb"))
        .collect();
    models.sort();
    assert!(!models.is_empty());

    for path in models {
        let model = load_gltf(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(!model.nodes.is_empty(), "{} has no nodes", path.display());
        assert!(!model.meshes.is_empty(), "{} has no meshes", path.display());
        let primitives: usize = model.nodes.iter().map(|node| node.primitives.len()).sum();
        println!(
            "{}: nodes {}, primitives {}, meshes {}, materials {}, textures {}, skins {}, animations {}",
            path.file_name().unwrap_or_default().to_string_lossy(),
            model.nodes.len(),
            primitives,
            model.meshes.len(),
            model.materials.len(),
            model.textures.len(),
            model.skins.len(),
            model.animations.len(),
        );
    }
}
