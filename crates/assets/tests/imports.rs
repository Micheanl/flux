use std::path::PathBuf;

use assets::{load_gltf, load_image};

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("flux_asset_test_{name}"));
    path
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let value = (b0 << 16) | (b1 << 8) | b2;
        output.push(TABLE[((value >> 18) & 63) as usize] as char);
        output.push(TABLE[((value >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[((value >> 6) & 63) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(value & 63) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

#[test]
fn loads_minimal_gltf() {
    let mut bytes = Vec::new();
    for position in [[-1.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
        for value in position {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    for index in [0u16, 1, 2] {
        bytes.extend_from_slice(&index.to_le_bytes());
    }

    let json = format!(
        r#"{{
  "asset": {{ "version": "2.0" }},
  "scene": 0,
  "scenes": [ {{ "nodes": [0] }} ],
  "nodes": [ {{ "mesh": 0, "name": "Triangle", "translation": [0.0, 2.0, 0.0] }} ],
  "meshes": [ {{ "primitives": [ {{ "attributes": {{ "POSITION": 0 }}, "indices": 1, "material": 0 }} ] }} ],
  "materials": [ {{ "pbrMetallicRoughness": {{ "baseColorFactor": [1.0, 0.2, 0.1, 1.0] }} }} ],
  "accessors": [
    {{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
       "min": [-1.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] }},
    {{ "bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR" }}
  ],
  "bufferViews": [
    {{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }},
    {{ "buffer": 0, "byteOffset": 36, "byteLength": 6 }}
  ],
  "buffers": [ {{ "byteLength": {}, "uri": "data:application/octet-stream;base64,{}" }} ]
}}"#,
        bytes.len(),
        base64_encode(&bytes)
    );

    let path = temp_path("minimal.gltf");
    std::fs::write(&path, json).unwrap();

    let model = load_gltf(&path).expect("glTF loading failed");
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

    let _ = std::fs::remove_file(path);
}

#[test]
fn loads_png_image() {
    let path = temp_path("image.png");
    let image = image::RgbaImage::from_raw(
        2,
        2,
        vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ],
    )
    .unwrap();
    image.save(&path).unwrap();

    let texture = load_image(&path).expect("image loading failed");
    assert_eq!(texture.width, 2);
    assert_eq!(texture.height, 2);
    assert_eq!(texture.rgba.len(), 16);
    assert_eq!(&texture.rgba[0..4], &[255, 0, 0, 255]);
    let _ = std::fs::remove_file(path);
}

#[test]
fn loads_animation_tracks() {
    let mut bytes = Vec::new();
    for position in [[-1.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
        for value in position {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    for index in [0u16, 1, 2] {
        bytes.extend_from_slice(&index.to_le_bytes());
    }
    for time in [0.0f32, 1.0] {
        bytes.extend_from_slice(&time.to_le_bytes());
    }
    for translation in [[0.0f32, 0.0, 0.0], [0.0, 3.0, 0.0]] {
        for value in translation {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }

    let json = format!(
        r#"{{
  "asset": {{ "version": "2.0" }},
  "scene": 0,
  "scenes": [ {{ "nodes": [0] }} ],
  "nodes": [ {{ "mesh": 0, "name": "Triangle" }} ],
  "meshes": [ {{ "primitives": [ {{ "attributes": {{ "POSITION": 0 }}, "indices": 1, "material": 0 }} ] }} ],
  "materials": [ {{ "pbrMetallicRoughness": {{ "baseColorFactor": [1.0, 1.0, 1.0, 1.0] }} }} ],
  "animations": [
    {{
      "name": "bob",
      "channels": [ {{ "sampler": 0, "target": {{ "node": 0, "path": "translation" }} }} ],
      "samplers": [ {{ "input": 2, "output": 3, "interpolation": "LINEAR" }} ]
    }}
  ],
  "accessors": [
    {{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
       "min": [-1.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] }},
    {{ "bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR" }},
    {{ "bufferView": 2, "componentType": 5126, "count": 2, "type": "SCALAR",
       "min": [0.0], "max": [1.0] }},
    {{ "bufferView": 3, "componentType": 5126, "count": 2, "type": "VEC3" }}
  ],
  "bufferViews": [
    {{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }},
    {{ "buffer": 0, "byteOffset": 36, "byteLength": 6 }},
    {{ "buffer": 0, "byteOffset": 42, "byteLength": 8 }},
    {{ "buffer": 0, "byteOffset": 50, "byteLength": 24 }}
  ],
  "buffers": [ {{ "byteLength": {}, "uri": "data:application/octet-stream;base64,{}" }} ]
}}"#,
        bytes.len(),
        base64_encode(&bytes)
    );

    let path = temp_path("animated.gltf");
    std::fs::write(&path, json).unwrap();

    let model = load_gltf(&path).expect("glTF loading failed");
    assert_eq!(model.animations.len(), 1);
    let clip = &model.animations[0];
    assert_eq!(clip.name, "bob");
    assert!((clip.duration - 1.0).abs() < 1e-5);
    assert_eq!(clip.tracks.len(), 1);
    let poses = clip.sample(0.5);
    assert_eq!(poses.len(), 1);
    let translation = poses[0].translation.expect("missing translation");
    assert!((translation - foundation::Vec3::new(0.0, 1.5, 0.0)).length() < 1e-5);
    let _ = std::fs::remove_file(path);
}
