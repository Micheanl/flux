use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Default for Aabb {
    fn default() -> Self {
        Self::invalid()
    }
}

impl Aabb {
    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_center_extents(center: Vec3, extents: Vec3) -> Self {
        Self {
            min: center - extents,
            max: center + extents,
        }
    }

    pub fn from_points(points: impl IntoIterator<Item = Vec3>) -> Self {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for point in points {
            min = min.min(point);
            max = max.max(point);
        }
        Self { min, max }
    }

    pub fn invalid() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.min.x <= self.max.x && self.min.y <= self.max.y && self.min.z <= self.max.z
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    pub fn transform(&self, matrix: &Mat4) -> Self {
        let center = matrix.transform_point3(self.center());
        let extents = self.extents();
        let basis = Mat3::from_mat4(*matrix);
        let world_extents = basis.x_axis.abs() * extents.x
            + basis.y_axis.abs() * extents.y
            + basis.z_axis.abs() * extents.z;
        Self::from_center_extents(center, world_extents)
    }

    pub fn union(&self, other: &Self) -> Self {
        if !self.is_valid() {
            return *other;
        }
        if !other.is_valid() {
            return *self;
        }
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    pub fn corners(&self) -> [Vec3; 8] {
        [
            Vec3::new(self.min.x, self.min.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.min.z),
            Vec3::new(self.min.x, self.max.y, self.min.z),
            Vec3::new(self.max.x, self.max.y, self.min.z),
            Vec3::new(self.min.x, self.min.y, self.max.z),
            Vec3::new(self.max.x, self.min.y, self.max.z),
            Vec3::new(self.min.x, self.max.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.max.z),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_keeps_bounds() {
        let aabb = Aabb::from_center_extents(Vec3::ZERO, Vec3::ONE);
        let matrix = Mat4::from_scale_rotation_translation(
            Vec3::splat(2.0),
            glam::Quat::from_rotation_y(0.7),
            Vec3::new(3.0, 4.0, 5.0),
        );
        let transformed = aabb.transform(&matrix);
        for corner in aabb.corners() {
            let world = matrix.transform_point3(corner);
            assert!(
                world.cmpge(transformed.min).all() && world.cmple(transformed.max).all(),
                "{world:?} is outside {transformed:?}"
            );
        }
    }
}
