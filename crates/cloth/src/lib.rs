use foundation::{Vec2, Vec3};

const DAMPING: f32 = 0.985;

#[derive(Clone, Copy, Debug)]
struct Link {
    a: u32,
    b: u32,
    rest: f32,
}

pub struct Cloth {
    width: usize,
    height: usize,
    positions: Vec<Vec3>,
    previous: Vec<Vec3>,
    pinned: Vec<bool>,
    links: Vec<Link>,
    indices: Vec<u32>,
}

impl Cloth {
    pub fn new(width: usize, height: usize, size: Vec2) -> Self {
        let width = width.max(2);
        let height = height.max(2);
        let spacing = Vec2::new(size.x / (width - 1) as f32, size.y / (height - 1) as f32);
        let mut positions = Vec::with_capacity(width * height);
        let mut pinned = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                positions.push(Vec3::new(
                    x as f32 * spacing.x,
                    -(y as f32) * spacing.y,
                    0.0,
                ));
                pinned.push(x == 0);
            }
        }
        let mut links = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let index = y * width + x;
                if x + 1 < width {
                    links.push(Link {
                        a: index as u32,
                        b: (index + 1) as u32,
                        rest: spacing.x,
                    });
                }
                if y + 1 < height {
                    links.push(Link {
                        a: index as u32,
                        b: (index + width) as u32,
                        rest: spacing.y,
                    });
                }
                if x + 1 < width && y + 1 < height {
                    let diagonal = (spacing.x * spacing.x + spacing.y * spacing.y).sqrt();
                    links.push(Link {
                        a: index as u32,
                        b: (index + width + 1) as u32,
                        rest: diagonal,
                    });
                    links.push(Link {
                        a: (index + 1) as u32,
                        b: (index + width) as u32,
                        rest: diagonal,
                    });
                }
            }
        }
        let mut indices = Vec::with_capacity((width - 1) * (height - 1) * 6);
        for y in 0..height - 1 {
            for x in 0..width - 1 {
                let a = (y * width + x) as u32;
                let b = a + 1;
                let c = a + width as u32;
                let d = c + 1;
                indices.extend_from_slice(&[a, d, b, a, c, d]);
            }
        }
        let previous = positions.clone();
        Self {
            width,
            height,
            positions,
            previous,
            pinned,
            links,
            indices,
        }
    }

    pub fn step(
        &mut self,
        dt: f32,
        acceleration: Vec3,
        iterations: u32,
        collision: Option<(Vec3, f32)>,
    ) {
        let dt = dt.clamp(0.0, 1.0 / 30.0);
        if dt == 0.0 {
            return;
        }
        for index in 0..self.positions.len() {
            if self.pinned[index] {
                continue;
            }
            let velocity = (self.positions[index] - self.previous[index]) * DAMPING;
            self.previous[index] = self.positions[index];
            self.positions[index] += velocity + acceleration * dt * dt;
        }
        for _ in 0..iterations.max(1) {
            self.solve_links();
        }
        if let Some((center, radius)) = collision {
            for index in 0..self.positions.len() {
                if self.pinned[index] {
                    continue;
                }
                let offset = self.positions[index] - center;
                let distance = offset.length();
                if distance < radius && distance > 1e-4 {
                    self.positions[index] = center + offset / distance * radius;
                }
            }
        }
    }

    fn solve_links(&mut self) {
        for link in &self.links {
            let a = link.a as usize;
            let b = link.b as usize;
            let delta = self.positions[b] - self.positions[a];
            let distance = delta.length();
            if distance < 1e-5 {
                continue;
            }
            let correction = delta * ((distance - link.rest) / distance);
            let (weight_a, weight_b) = match (self.pinned[a], self.pinned[b]) {
                (true, true) => continue,
                (true, false) => (0.0, 1.0),
                (false, true) => (1.0, 0.0),
                (false, false) => (0.5, 0.5),
            };
            self.positions[a] += correction * weight_a;
            self.positions[b] -= correction * weight_b;
        }
    }

    pub fn positions(&self) -> &[Vec3] {
        &self.positions
    }

    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spread(cloth: &Cloth) -> f32 {
        let first = cloth.positions()[0];
        cloth
            .positions()
            .iter()
            .map(|position| position.distance(first))
            .fold(0.0, f32::max)
    }

    #[test]
    fn pinned_edge_stays() {
        let mut cloth = Cloth::new(8, 6, Vec2::new(4.0, 3.0));
        let pinned: Vec<Vec3> = (0..6).map(|y| cloth.positions()[y * 8]).collect();
        for _ in 0..60 {
            cloth.step(1.0 / 60.0, Vec3::new(0.0, -9.8, 0.0), 4, None);
        }
        for (y, position) in pinned.iter().enumerate() {
            assert!(cloth.positions()[y * 8].distance(*position) < 1e-4);
        }
    }

    #[test]
    fn cloth_keeps_its_extent() {
        let mut cloth = Cloth::new(10, 8, Vec2::new(4.0, 3.0));
        for _ in 0..120 {
            cloth.step(1.0 / 60.0, Vec3::new(0.0, -9.8, 0.0), 6, None);
        }
        let diagonal = (4.0f32 * 4.0 + 3.0 * 3.0).sqrt();
        let extent = spread(&cloth);
        assert!(extent <= diagonal + 0.2, "{extent}");
    }

    #[test]
    fn wind_displaces_cloth() {
        let mut cloth = Cloth::new(10, 8, Vec2::new(4.0, 3.0));
        let mut still: Vec<Vec3> = cloth.positions().to_vec();
        for _ in 0..40 {
            cloth.step(1.0 / 60.0, Vec3::new(0.0, -9.8, 0.0), 4, None);
        }
        still.clone_from(&cloth.positions().to_vec());
        for _ in 0..40 {
            cloth.step(1.0 / 60.0, Vec3::new(0.0, -9.8, 40.0), 4, None);
        }
        let drift = cloth
            .positions()
            .iter()
            .zip(&still)
            .map(|(now, before)| (now.z - before.z).abs())
            .fold(0.0, f32::max);
        assert!(drift > 0.2, "{drift}");
    }

    #[test]
    fn sphere_collision_pushes_out() {
        let mut cloth = Cloth::new(8, 8, Vec2::new(3.0, 3.0));
        let center = Vec3::new(1.5, -1.5, -1.0);
        for _ in 0..120 {
            cloth.step(
                1.0 / 60.0,
                Vec3::new(0.0, -9.8, 0.0),
                6,
                Some((center, 0.6)),
            );
        }
        let inside = cloth
            .positions()
            .iter()
            .filter(|position| position.distance(center) < 0.55)
            .count();
        assert_eq!(inside, 0);
    }
}
