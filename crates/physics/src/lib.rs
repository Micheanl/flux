mod character;
mod obb;

pub use character::CharacterController;
pub use obb::Obb;

#[cfg(test)]
mod ray_tests {
    use super::*;
    use foundation::{Quat, Vec3};

    #[test]
    fn ray_hits_box() {
        let obb = Obb::new(Vec3::ZERO, Vec3::splat(0.5), Quat::IDENTITY);
        let hit = obb.ray_intersection(Vec3::new(0.0, 0.0, 3.0), Vec3::NEG_Z);
        assert!((hit.expect("hit") - 2.5).abs() < 1e-4);
    }

    #[test]
    fn ray_misses_box() {
        let obb = Obb::new(Vec3::ZERO, Vec3::splat(0.5), Quat::IDENTITY);
        assert!(
            obb.ray_intersection(Vec3::new(3.0, 3.0, 3.0), Vec3::NEG_Z)
                .is_none()
        );
    }
}
