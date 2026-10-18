use foundation::Vec3;

use crate::mesh::{MeshData, Vertex};

pub fn cube(size: f32) -> MeshData {
    let half = size * 0.5;
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);

    let faces: [([f32; 3], [f32; 3], [f32; 4]); 6] = [
        ([0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0]),
        ([0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [-1.0, 0.0, 0.0, 1.0]),
        ([1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0, 1.0]),
        ([-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 1.0]),
        ([0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0, 1.0]),
        ([0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0, 1.0]),
    ];

    for (normal, normal_out, tangent) in faces {
        let normal = Vec3::from_array(normal);
        let right = Vec3::new(tangent[0], tangent[1], tangent[2]).normalize();
        let up = normal.cross(right).normalize();
        let center = normal * half;
        let corners = [
            center - right * half - up * half,
            center + right * half - up * half,
            center + right * half + up * half,
            center - right * half + up * half,
        ];
        let uvs = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let base = vertices.len() as u32;
        for (corner, uv) in corners.iter().zip(uvs) {
            vertices.push(Vertex::new(
                corner.to_array(),
                normal_out,
                [uv.0, uv.1],
                tangent,
            ));
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    MeshData::new(vertices, indices)
}

pub fn plane(size: f32, segments: u32) -> MeshData {
    let segments = segments.max(1);
    let step = size / segments as f32;
    let half = size * 0.5;
    let mut vertices = Vec::with_capacity(((segments + 1) * (segments + 1)) as usize);
    for z in 0..=segments {
        for x in 0..=segments {
            let px = -half + x as f32 * step;
            let pz = -half + z as f32 * step;
            vertices.push(Vertex::new(
                [px, 0.0, pz],
                [0.0, 1.0, 0.0],
                [x as f32 / segments as f32, z as f32 / segments as f32],
                [1.0, 0.0, 0.0, 1.0],
            ));
        }
    }
    let mut indices = Vec::with_capacity((segments * segments * 6) as usize);
    let row = segments + 1;
    for z in 0..segments {
        for x in 0..segments {
            let top_left = z * row + x;
            let top_right = top_left + 1;
            let bottom_left = top_left + row;
            let bottom_right = bottom_left + 1;
            indices.extend_from_slice(&[
                top_left,
                bottom_left,
                top_right,
                top_right,
                bottom_left,
                bottom_right,
            ]);
        }
    }
    MeshData::new(vertices, indices)
}

pub fn uv_sphere(radius: f32, segments: u32, rings: u32) -> MeshData {
    let segments = segments.max(3);
    let rings = rings.max(2);
    let mut vertices = Vec::new();
    for ring in 0..=rings {
        let v = ring as f32 / rings as f32;
        let phi = v * std::f32::consts::PI;
        for segment in 0..=segments {
            let u = segment as f32 / segments as f32;
            let theta = u * std::f32::consts::TAU;
            let normal = Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
            let position = normal * radius;
            let tangent = Vec3::new(-theta.sin(), 0.0, theta.cos()).normalize_or_zero();
            vertices.push(Vertex::new(
                position.to_array(),
                normal.to_array(),
                [u, v],
                tangent.extend(1.0).to_array(),
            ));
        }
    }

    let mut indices = Vec::new();
    let row = segments + 1;
    for ring in 0..rings {
        for segment in 0..segments {
            let a = ring * row + segment;
            let b = a + row;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    MeshData::new(vertices, indices)
}

pub fn torus(
    major_radius: f32,
    minor_radius: f32,
    major_segments: u32,
    minor_segments: u32,
) -> MeshData {
    let major_segments = major_segments.max(3);
    let minor_segments = minor_segments.max(3);
    let mut vertices = Vec::new();
    for major in 0..=major_segments {
        let u = major as f32 / major_segments as f32;
        let theta = u * std::f32::consts::TAU;
        let center = Vec3::new(theta.cos() * major_radius, 0.0, theta.sin() * major_radius);
        for minor in 0..=minor_segments {
            let v = minor as f32 / minor_segments as f32;
            let phi = v * std::f32::consts::TAU;
            let normal = Vec3::new(theta.cos() * phi.cos(), phi.sin(), theta.sin() * phi.cos());
            let position = center + normal * minor_radius;
            let tangent = Vec3::new(-theta.sin(), 0.0, theta.cos()).normalize_or_zero();
            vertices.push(Vertex::new(
                position.to_array(),
                normal.to_array(),
                [u, v],
                tangent.extend(1.0).to_array(),
            ));
        }
    }
    let mut indices = Vec::new();
    let row = minor_segments + 1;
    for major in 0..major_segments {
        for minor in 0..minor_segments {
            let a = major * row + minor;
            let b = a + row;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    MeshData::new(vertices, indices)
}
