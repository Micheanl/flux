use foundation::{Aabb, Mat4, Quat, Transform, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct Obb {
    pub center: Vec3,
    pub half_extents: Vec3,
    pub rotation: Quat,
}

impl Obb {
    pub fn new(center: Vec3, half_extents: Vec3, rotation: Quat) -> Self {
        Self {
            center,
            half_extents,
            rotation,
        }
    }

    pub fn from_transform(transform: &Transform, bounds: &Aabb) -> Self {
        Self::from_matrix(&transform.matrix(), bounds)
    }

    pub fn from_matrix(matrix: &Mat4, bounds: &Aabb) -> Self {
        let (scale, rotation, _) = matrix.to_scale_rotation_translation();
        let center = matrix.transform_point3(bounds.center());
        let extents = bounds.extents() * scale.abs();
        Self {
            center,
            half_extents: extents,
            rotation,
        }
    }

    pub fn contains(&self, point: Vec3) -> bool {
        self.local_point(point).abs().cmpge(self.half_extents).all()
    }

    pub fn closest_point(&self, point: Vec3) -> Vec3 {
        let local = self
            .local_point(point)
            .clamp(-self.half_extents, self.half_extents);
        self.rotation * local + self.center
    }

    fn local_point(&self, point: Vec3) -> Vec3 {
        self.rotation.inverse() * (point - self.center)
    }

    pub fn push_out_sphere(&self, center: Vec3, radius: f32) -> Option<Vec3> {
        let local = self.local_point(center);
        let clamped = local.clamp(-self.half_extents, self.half_extents);
        let delta = local - clamped;
        let distance_squared = delta.length_squared();
        let normal = if distance_squared > 1e-10 {
            if distance_squared >= radius * radius {
                return None;
            }
            delta / distance_squared.sqrt()
        } else {
            let gaps = self.half_extents - local.abs();
            let axis = if gaps.x <= gaps.y && gaps.x <= gaps.z {
                0
            } else if gaps.y <= gaps.z {
                1
            } else {
                2
            };
            let mut normal = Vec3::ZERO;
            normal[axis] = if local[axis] >= 0.0 { 1.0 } else { -1.0 };
            let penetration = radius + gaps[axis];
            return Some(self.rotation * normal * penetration);
        };
        let penetration = radius - distance_squared.sqrt();
        Some(self.rotation * normal * penetration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::Vec3;

    #[test]
    fn sphere_outside_box_is_pushed_away() {
        let obb = Obb::new(Vec3::ZERO, Vec3::splat(1.0), Quat::IDENTITY);
        let push = obb.push_out_sphere(Vec3::new(1.5, 0.0, 0.0), 1.0);
        let push = push.expect("sphere overlaps box");
        assert!((push.x - 0.5).abs() < 1e-4);
    }

    #[test]
    fn distant_sphere_does_not_collide() {
        let obb = Obb::new(Vec3::ZERO, Vec3::splat(1.0), Quat::IDENTITY);
        assert!(obb.push_out_sphere(Vec3::new(5.0, 0.0, 0.0), 1.0).is_none());
    }

    #[test]
    fn rotated_box_push_is_oriented() {
        let obb = Obb::new(
            Vec3::ZERO,
            Vec3::new(2.0, 0.5, 0.5),
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        );
        let push = obb
            .push_out_sphere(Vec3::new(0.0, 0.0, 2.4), 0.5)
            .expect("sphere overlaps rotated box");
        assert!((push.z - 0.1).abs() < 1e-3, "{push:?}");
    }
}
