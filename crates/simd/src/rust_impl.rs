use foundation::{Aabb, Frustum, Mat4, Vec3};

pub fn frustum_cull(frustum: &Frustum, aabbs: &[Aabb], mask: &mut [u8]) -> u32 {
    debug_assert_eq!(aabbs.len(), mask.len());
    let mut visible = 0;
    for (slot, aabb) in mask.iter_mut().zip(aabbs) {
        let inside = frustum.intersects_aabb(aabb);
        *slot = inside as u8;
        visible += inside as u32;
    }
    visible
}

pub fn aabb_transform(matrix: &Mat4, aabbs: &[Aabb], out: &mut [Aabb]) {
    debug_assert_eq!(aabbs.len(), out.len());
    for (dst, src) in out.iter_mut().zip(aabbs) {
        *dst = src.transform(matrix);
    }
}

pub fn mat4_mul(lhs: &Mat4, rhs: &Mat4) -> Mat4 {
    *lhs * *rhs
}

pub fn transform_points(matrix: &Mat4, points: &[Vec3], out: &mut [Vec3]) {
    debug_assert_eq!(points.len(), out.len());
    for (dst, point) in out.iter_mut().zip(points) {
        *dst = matrix.transform_point3(*point);
    }
}

pub fn simd_width() -> u32 {
    4
}
