use glam::Mat4;

use crate::frustum::Frustum;
use crate::math;
use crate::transform::Transform;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub view: Mat4,
    pub projection: Mat4,
    pub view_projection: Mat4,
    pub inverse_view_projection: Mat4,
    pub position: glam::Vec3,
    pub near: f32,
}

impl Camera {
    pub fn perspective(transform: &Transform, fov_y: f32, aspect: f32, near: f32) -> Self {
        let view = transform.inverse_matrix();
        let projection = math::perspective_infinite_reverse_z_rh(fov_y, aspect, near);
        let view_projection = projection * view;
        Self {
            view,
            projection,
            view_projection,
            inverse_view_projection: view_projection.inverse(),
            position: transform.translation,
            near,
        }
    }

    pub fn frustum(&self) -> Frustum {
        Frustum::from_view_projection(&self.view_projection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn projects_geometry_in_front() {
        let transform = Transform::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let camera = Camera::perspective(&transform, 1.0, 16.0 / 9.0, 0.1);
        let point = transform.translation + Vec3::new(0.0, 0.0, -10.0);
        let clip = camera.view_projection.project_point3(point);
        assert!((0.0..1.0).contains(&clip.z));
    }
}
