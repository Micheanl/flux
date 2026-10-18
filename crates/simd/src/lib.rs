#[cfg(zig_native)]
mod native;
#[cfg(any(not(zig_native), test))]
#[cfg_attr(zig_native, allow(dead_code))]
mod rust_impl;

#[cfg(zig_native)]
pub use native::*;
#[cfg(not(zig_native))]
pub use rust_impl::*;

pub const ZIG_ACCELERATED: bool = cfg!(zig_native);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rust_impl;
    use foundation::{Aabb, Camera, EulerRot, Frustum, Mat4, Quat, Transform, Vec3};

    fn fuzz_aabbs(count: usize) -> Vec<Aabb> {
        let mut seed = 0x1234_5678_9abc_def0u64;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) as f32 / (1u32 << 31) as f32) * 40.0 - 20.0
        };
        (0..count)
            .map(|_| {
                let center = Vec3::new(next(), next(), next());
                let extents = Vec3::new(next().abs() * 0.2, next().abs() * 0.2, next().abs() * 0.2);
                Aabb::from_center_extents(center, extents)
            })
            .collect()
    }

    #[test]
    fn cull_matches_reference() {
        let camera = Camera::perspective(
            &Transform::from_translation(Vec3::new(0.0, 0.0, 6.0)),
            1.0,
            16.0 / 9.0,
            0.1,
        );
        let frustum: Frustum = camera.frustum();
        let aabbs = fuzz_aabbs(97);
        let mut mask = vec![0u8; aabbs.len()];
        let visible = frustum_cull(&frustum, &aabbs, &mut mask);

        let mut reference = vec![0u8; aabbs.len()];
        let expected = rust_impl::frustum_cull(&frustum, &aabbs, &mut reference);
        assert_eq!(mask, reference);
        assert_eq!(visible, expected);
    }

    #[test]
    fn aabb_transform_matches_reference() {
        let matrix = Mat4::from_scale_rotation_translation(
            Vec3::splat(1.7),
            Quat::from_rotation_y(0.4),
            Vec3::new(2.0, -3.0, 1.0),
        );
        let aabbs = fuzz_aabbs(31);
        let mut ours = vec![Aabb::invalid(); aabbs.len()];
        let mut reference = vec![Aabb::invalid(); aabbs.len()];
        aabb_transform(&matrix, &aabbs, &mut ours);
        rust_impl::aabb_transform(&matrix, &aabbs, &mut reference);
        for (a, b) in ours.iter().zip(&reference) {
            assert!((a.min - b.min).length() < 1e-3, "{a:?} vs {b:?}");
            assert!((a.max - b.max).length() < 1e-3, "{a:?} vs {b:?}");
        }
    }

    #[test]
    fn mat4_mul_matches_reference() {
        let a = Mat4::from_rotation_translation(
            Quat::from_euler(EulerRot::XYZ, 0.1, 0.2, 0.3),
            Vec3::new(1.0, 2.0, 3.0),
        );
        let b = Mat4::from_scale(Vec3::new(2.0, 0.5, 1.5));
        let got = mat4_mul(&a, &b);
        let delta: f32 = (got - a * b).to_cols_array().iter().map(|v| v.abs()).sum();
        assert!(delta < 1e-4);
    }

    #[test]
    fn transform_points_matches_reference() {
        let matrix = Mat4::from_rotation_y(0.7);
        let points: Vec<Vec3> = (0..40)
            .map(|i| Vec3::new(i as f32 * 0.3, -(i as f32), 2.0))
            .collect();
        let mut ours = vec![Vec3::ZERO; points.len()];
        let mut reference = vec![Vec3::ZERO; points.len()];
        transform_points(&matrix, &points, &mut ours);
        rust_impl::transform_points(&matrix, &points, &mut reference);
        for (a, b) in ours.iter().zip(&reference) {
            assert!((*a - *b).length() < 1e-3);
        }
    }

    #[test]
    fn vector_width_is_sane() {
        assert!(simd_width() >= 4);
    }
}
