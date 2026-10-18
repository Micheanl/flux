use foundation::{Vec3, Vec3Swizzles};

use crate::obb::Obb;

const GRAVITY: f32 = 22.0;

#[derive(Clone, Copy, Debug)]
pub struct CharacterController {
    pub position: Vec3,
    pub velocity: Vec3,
    pub radius: f32,
    pub height: f32,
    pub grounded: bool,
    pub ground_normal: Vec3,
}

impl CharacterController {
    pub fn new(position: Vec3, radius: f32, height: f32) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            radius,
            height: height.max(radius * 2.0),
            grounded: false,
            ground_normal: Vec3::Y,
        }
    }

    pub fn eye(&self) -> Vec3 {
        self.position + Vec3::Y * (self.height - self.radius * 0.8)
    }

    pub fn update(
        &mut self,
        colliders: &[Obb],
        floor_height: f32,
        wish_direction: Vec3,
        speed: f32,
        jump_speed: f32,
        dt: f32,
    ) {
        let dt = dt.clamp(0.0, 0.1);
        let wish = wish_direction.xz().clamp_length_max(1.0);
        let target = Vec3::new(wish.x, 0.0, wish.y) * speed;
        let horizontal = self.velocity.xz();
        let acceleration = if self.grounded { 18.0 } else { 6.0 };
        let delta = target.xz() - horizontal;
        let max_delta = acceleration * dt;
        let horizontal = horizontal + delta.clamp_length_max(max_delta);
        self.velocity.x = horizontal.x;
        self.velocity.z = horizontal.y;
        self.velocity.y = (self.velocity.y - GRAVITY * dt).max(-60.0);

        if jump_speed > 0.0 && self.grounded {
            self.velocity.y = jump_speed;
            self.grounded = false;
        }

        self.position += self.velocity * dt;
        self.resolve(colliders);

        if self.position.y <= floor_height {
            self.position.y = floor_height;
            if self.velocity.y < 0.0 {
                self.velocity.y = 0.0;
            }
            self.grounded = true;
            self.ground_normal = Vec3::Y;
        }

        if self.grounded && self.velocity.y < 0.0 {
            self.velocity.y = 0.0;
        }
    }

    fn resolve(&mut self, colliders: &[Obb]) {
        let offset = self.radius;
        let span = (self.height - self.radius * 2.0).max(0.0);
        let samples = 4;
        self.grounded = false;
        self.ground_normal = Vec3::Y;

        for _ in 0..4 {
            let mut pushed = false;
            for collider in colliders {
                for sample in 0..=samples {
                    let t = sample as f32 / samples as f32;
                    let center = self.position + Vec3::Y * (offset + span * t);
                    let Some(push) = collider.push_out_sphere(center, self.radius) else {
                        continue;
                    };
                    self.position += push;
                    pushed = true;
                    if push.normalize_or_zero().y > 0.5 {
                        self.grounded = true;
                        self.ground_normal = Vec3::Y;
                        if self.velocity.y < 0.0 {
                            self.velocity.y = 0.0;
                        }
                    }
                }
            }
            if !pushed {
                break;
            }
        }
        if self.grounded {
            self.velocity.y = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::Quat;

    fn box_at(x: f32, z: f32, half: f32) -> Obb {
        Obb::new(Vec3::new(x, half, z), Vec3::splat(half), Quat::IDENTITY)
    }

    #[test]
    fn falls_onto_floor() {
        let mut controller = CharacterController::new(Vec3::new(0.0, 3.0, 0.0), 0.4, 1.8);
        for _ in 0..240 {
            controller.update(&[], 0.0, Vec3::ZERO, 6.0, 0.0, 1.0 / 60.0);
        }
        assert!(controller.grounded);
        assert!(controller.position.y.abs() < 1e-3);
    }

    #[test]
    fn walks_and_stops_at_wall() {
        let mut controller = CharacterController::new(Vec3::new(0.0, 0.0, 0.0), 0.4, 1.8);
        for _ in 0..180 {
            controller.update(&[], 0.0, Vec3::ZERO, 6.0, 0.0, 1.0 / 60.0);
        }
        let wall = box_at(2.0, 0.0, 0.5);
        for _ in 0..240 {
            controller.update(&[wall], 0.0, Vec3::X, 6.0, 0.0, 1.0 / 60.0);
        }
        assert!(controller.position.x < 2.0 - 0.5);
        assert!(controller.position.x > 0.8, "{}", controller.position.x);
    }

    #[test]
    fn jumps_when_grounded() {
        let mut controller = CharacterController::new(Vec3::new(0.0, 0.0, 0.0), 0.4, 1.8);
        controller.update(&[], 0.0, Vec3::ZERO, 6.0, 0.0, 1.0 / 60.0);
        controller.update(&[], 0.0, Vec3::ZERO, 6.0, 7.0, 1.0 / 60.0);
        assert!(!controller.grounded);
        assert!(controller.velocity.y > 0.0);
    }
}
