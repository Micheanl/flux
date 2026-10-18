use bytemuck::cast_slice;
use foundation::{Aabb, Frustum, Mat4, Vec3};

unsafe extern "C" {
    #[link_name = "frustum_cull"]
    fn raw_frustum_cull(mask: *mut u8, aabbs: *const f32, planes: *const f32, count: u32) -> u32;
    #[link_name = "aabb_transform"]
    fn raw_aabb_transform(out: *mut f32, aabbs: *const f32, matrix: *const f32, count: u32);
    #[link_name = "mat4_mul"]
    fn raw_mat4_mul(out: *mut f32, lhs: *const f32, rhs: *const f32);
    #[link_name = "transform_points"]
    fn raw_transform_points(out: *mut f32, matrix: *const f32, points: *const f32, count: u32);
    #[link_name = "simd_width"]
    fn raw_simd_width() -> u32;
}

pub fn frustum_cull(frustum: &Frustum, aabbs: &[Aabb], mask: &mut [u8]) -> u32 {
    debug_assert_eq!(aabbs.len(), mask.len());
    if aabbs.is_empty() {
        return 0;
    }
    let planes = cast_slice::<_, f32>(&frustum.planes);
    let bounds = cast_slice::<_, f32>(aabbs);

    unsafe {
        raw_frustum_cull(
            mask.as_mut_ptr(),
            bounds.as_ptr(),
            planes.as_ptr(),
            mask.len() as u32,
        )
    }
}

pub fn aabb_transform(matrix: &Mat4, aabbs: &[Aabb], out: &mut [Aabb]) {
    debug_assert_eq!(aabbs.len(), out.len());
    if aabbs.is_empty() {
        return;
    }
    let matrix = matrix.to_cols_array();
    let bounds = cast_slice::<_, f32>(aabbs);

    unsafe {
        raw_aabb_transform(
            out.as_mut_ptr() as *mut f32,
            bounds.as_ptr(),
            matrix.as_ptr(),
            out.len() as u32,
        )
    }
}

pub fn mat4_mul(lhs: &Mat4, rhs: &Mat4) -> Mat4 {
    let a = lhs.to_cols_array();
    let b = rhs.to_cols_array();
    let mut out = [0.0f32; 16];

    unsafe { raw_mat4_mul(out.as_mut_ptr(), a.as_ptr(), b.as_ptr()) };
    Mat4::from_cols_array(&out)
}

pub fn transform_points(matrix: &Mat4, points: &[Vec3], out: &mut [Vec3]) {
    debug_assert_eq!(points.len(), out.len());
    if points.is_empty() {
        return;
    }
    let matrix = matrix.to_cols_array();
    let points = cast_slice::<_, f32>(points);

    unsafe {
        raw_transform_points(
            out.as_mut_ptr() as *mut f32,
            matrix.as_ptr(),
            points.as_ptr(),
            out.len() as u32,
        )
    }
}

pub fn simd_width() -> u32 {
    unsafe { raw_simd_width() }
}
