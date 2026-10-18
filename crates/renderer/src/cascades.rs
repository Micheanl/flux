use foundation::{Mat4, Vec3, look_at_rh, orthographic_reverse_z_rh};

use crate::scene::CameraData;
use crate::uniforms::MAX_CASCADES;

pub struct Cascades {
    pub matrices: [Mat4; MAX_CASCADES],
    pub splits: [f32; MAX_CASCADES],
    pub world_texel: f32,
}

pub fn compute(
    camera: &CameraData,
    sun_direction: Vec3,
    count: u32,
    distance: f32,
    shadow_size: u32,
) -> Cascades {
    let count = count.clamp(1, MAX_CASCADES as u32) as usize;
    let near = camera.near.max(0.01);
    let far = distance.max(near + 1.0);
    let inverse_view_projection = camera.view_projection.inverse();
    let sun = sun_direction.normalize_or_zero();
    let up = if sun.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
    let light_rotation = look_at_rh(Vec3::ZERO, -sun, up);
    let lambda = 0.65;

    let mut splits = [far; MAX_CASCADES];
    let mut matrices = [Mat4::IDENTITY; MAX_CASCADES];
    let mut world_texel = 0.0;

    for index in 0..count {
        let fraction = (index + 1) as f32 / count as f32;
        let logarithmic = near * (far / near).powf(fraction);
        let uniform = near + (far - near) * fraction;
        let split_far = lambda * logarithmic + (1.0 - lambda) * uniform;
        splits[index] = split_far;

        let corners = frustum_corners(&inverse_view_projection, near, split_far);
        let center = corners.iter().copied().sum::<Vec3>() / 8.0;
        let radius = corners
            .iter()
            .map(|corner| corner.distance(center))
            .fold(0.0f32, f32::max)
            .max(1.0)
            * 1.02;
        let texel = 2.0 * radius / shadow_size.max(1) as f32;
        let center_light = light_rotation.transform_point3(center);
        let snapped = Vec3::new(
            (center_light.x / texel).round() * texel,
            (center_light.y / texel).round() * texel,
            center_light.z,
        );
        let shift = snapped - center_light;

        let view = look_at_rh(center + sun * radius * 2.0, center, up);
        let projection = orthographic_reverse_z_rh(
            shift.x - radius,
            shift.x + radius,
            shift.y - radius,
            shift.y + radius,
            radius * 0.5,
            radius * 3.0,
        );
        matrices[index] = projection * view;
        world_texel = texel;
    }

    Cascades {
        matrices,
        splits,
        world_texel,
    }
}

fn frustum_corners(inverse_view_projection: &Mat4, near: f32, split_far: f32) -> [Vec3; 8] {
    let mut corners = [Vec3::ZERO; 8];
    let far_z = (near / split_far).clamp(0.0, 1.0);
    for (index, (x, y)) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
        .into_iter()
        .enumerate()
    {
        corners[index] = inverse_view_projection.project_point3(Vec3::new(x, y, 1.0));
        corners[index + 4] = inverse_view_projection.project_point3(Vec3::new(x, y, far_z));
    }
    corners
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::CameraData;

    #[test]
    fn splits_are_monotonic_and_bounded() {
        let camera = CameraData::default();
        let cascades = compute(&camera, Vec3::Y, 4, 100.0, 2048);
        assert!(cascades.splits[0] > camera.near);
        for pair in cascades.splits.windows(2) {
            assert!(pair[1] >= pair[0]);
        }
    }

    #[test]
    fn cascade_is_stable_under_small_camera_shift() {
        let mut camera = CameraData {
            view_projection: Mat4::from_translation(Vec3::new(0.0, 0.0, -10.0)).inverse(),
            ..Default::default()
        };
        let first = compute(&camera, Vec3::new(0.3, 0.9, 0.2), 4, 80.0, 2048);
        camera.view_projection = Mat4::from_translation(Vec3::new(0.001, 0.0, -10.0)).inverse();
        let second = compute(&camera, Vec3::new(0.3, 0.9, 0.2), 4, 80.0, 2048);
        let delta = (first.matrices[0] - second.matrices[0]).to_cols_array();
        assert!(delta.iter().all(|value| value.abs() < 1e-3), "{delta:?}");
    }
}
