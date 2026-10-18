use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};

use crate::aabb::Aabb;
use crate::math::FRUSTUM_PLANES;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Plane {
    pub normal: Vec3,
    pub d: f32,
}

impl Plane {
    fn from_vec4(value: Vec4) -> Self {
        let normal = value.truncate();
        let length = normal.length();
        if length > 1e-8 {
            Self {
                normal: normal / length,
                d: value.w / length,
            }
        } else {
            Self {
                normal: Vec3::ZERO,
                d: f32::MAX,
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    pub planes: [Plane; FRUSTUM_PLANES],
}

impl Frustum {
    pub fn from_view_projection(view_projection: &Mat4) -> Self {
        let m = view_projection;
        let row0 = m.row(0);
        let row1 = m.row(1);
        let row2 = m.row(2);
        let row3 = m.row(3);
        Self {
            planes: [
                Plane::from_vec4(row3 + row0),
                Plane::from_vec4(row3 - row0),
                Plane::from_vec4(row3 + row1),
                Plane::from_vec4(row3 - row1),
                Plane::from_vec4(row2),
                Plane::from_vec4(row3 - row2),
            ],
        }
    }

    pub fn intersects_aabb(&self, aabb: &Aabb) -> bool {
        let center = aabb.center();
        let extents = aabb.extents();
        self.planes.iter().all(|plane| {
            let distance = plane.normal.dot(center) + plane.d;
            let radius = plane.normal.abs().dot(extents);
            distance + radius >= 0.0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{look_at_rh, perspective_infinite_reverse_z_rh};

    fn camera_view_projection() -> Mat4 {
        let view = look_at_rh(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        perspective_infinite_reverse_z_rh(1.0, 1.0, 0.1) * view
    }

    #[test]
    fn contains_geometry_in_front() {
        let frustum = Frustum::from_view_projection(&camera_view_projection());
        assert!(frustum.intersects_aabb(&Aabb::from_center_extents(Vec3::ZERO, Vec3::ONE)));
    }

    #[test]
    fn rejects_geometry_outside() {
        let frustum = Frustum::from_view_projection(&camera_view_projection());
        let behind = Aabb::from_center_extents(Vec3::new(0.0, 0.0, 12.0), Vec3::ONE);
        let outside = Aabb::from_center_extents(Vec3::new(20.0, 0.0, 0.0), Vec3::ONE);
        assert!(!frustum.intersects_aabb(&behind));
        assert!(!frustum.intersects_aabb(&outside));
    }
}
