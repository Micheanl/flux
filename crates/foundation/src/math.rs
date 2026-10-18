use glam::Mat4;
use glam::camera::rh::proj::directx;
use glam::camera::rh::view;

pub const FRUSTUM_PLANES: usize = 6;
pub const MAX_SHADOW_CASCADES: usize = 4;

pub fn perspective_infinite_reverse_z_rh(fov_y: f32, aspect: f32, near: f32) -> Mat4 {
    directx::perspective_infinite_reverse(fov_y, aspect, near)
}

pub fn orthographic_reverse_z_rh(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near: f32,
    far: f32,
) -> Mat4 {
    let projection = directx::orthographic(left, right, bottom, top, near, far);
    let flip = Mat4::from_cols(
        glam::Vec4::new(1.0, 0.0, 0.0, 0.0),
        glam::Vec4::new(0.0, 1.0, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, -1.0, 0.0),
        glam::Vec4::new(0.0, 0.0, 1.0, 1.0),
    );
    flip * projection
}

pub fn look_at_rh(eye: glam::Vec3, target: glam::Vec3, up: glam::Vec3) -> Mat4 {
    view::look_at_mat4(eye, target, up)
}
