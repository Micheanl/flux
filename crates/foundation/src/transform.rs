use glam::{Mat4, Quat, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            ..Self::IDENTITY
        }
    }

    pub fn looking_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let view = crate::math::look_at_rh(eye, target, up);
        let (_, rotation, _) = view.inverse().to_scale_rotation_translation();
        Self {
            translation: eye,
            rotation,
            scale: Vec3::ONE,
        }
    }

    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    pub fn inverse_matrix(&self) -> Mat4 {
        self.matrix().inverse()
    }

    pub fn forward(&self) -> Vec3 {
        self.rotation * Vec3::NEG_Z
    }
}
