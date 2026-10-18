use std::collections::HashMap;

use foundation::{Quat, Vec3};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interpolation {
    Linear,
    Step,
}

#[derive(Clone, Debug)]
pub enum TrackValues {
    Translations(Vec<Vec3>),
    Rotations(Vec<Quat>),
    Scales(Vec<Vec3>),
    Weights(Vec<Vec<f32>>),
}

#[derive(Clone, Debug)]
pub struct AnimationTrack {
    pub node: usize,
    pub interpolation: Interpolation,
    pub times: Vec<f32>,
    pub values: TrackValues,
}

#[derive(Clone, Debug, Default)]
pub struct AnimationClip {
    pub name: String,
    pub duration: f32,
    pub tracks: Vec<AnimationTrack>,
}

#[derive(Clone, Debug, Default)]
pub struct NodePose {
    pub node: usize,
    pub translation: Option<Vec3>,
    pub rotation: Option<Quat>,
    pub scale: Option<Vec3>,
    pub weights: Option<Vec<f32>>,
}

impl AnimationClip {
    pub fn sample(&self, time: f32) -> Vec<NodePose> {
        let mut poses: HashMap<usize, NodePose> = HashMap::new();
        for track in &self.tracks {
            let pose = poses.entry(track.node).or_insert(NodePose {
                node: track.node,
                ..Default::default()
            });
            match &track.values {
                TrackValues::Translations(values) => {
                    pose.translation = Some(sample_vectors(
                        &track.times,
                        values,
                        track.interpolation,
                        time,
                    ));
                }
                TrackValues::Rotations(values) => {
                    pose.rotation = Some(sample_rotations(
                        &track.times,
                        values,
                        track.interpolation,
                        time,
                    ));
                }
                TrackValues::Scales(values) => {
                    pose.scale = Some(sample_vectors(
                        &track.times,
                        values,
                        track.interpolation,
                        time,
                    ));
                }
                TrackValues::Weights(values) => {
                    pose.weights = Some(sample_weights(
                        &track.times,
                        values,
                        track.interpolation,
                        time,
                    ));
                }
            }
        }
        let mut result: Vec<NodePose> = poses.into_values().collect();
        result.sort_by_key(|pose| pose.node);
        result
    }
}

fn locate(times: &[f32], time: f32) -> (usize, usize, f32) {
    let next = times
        .partition_point(|&sample| sample <= time)
        .clamp(1, times.len() - 1);
    let previous = next - 1;
    let span = (times[next] - times[previous]).max(1e-6);
    (
        previous,
        next,
        ((time - times[previous]) / span).clamp(0.0, 1.0),
    )
}

fn sample_vectors(times: &[f32], values: &[Vec3], interpolation: Interpolation, time: f32) -> Vec3 {
    let Some(first) = values.first().copied() else {
        return Vec3::ZERO;
    };
    if times.len() < 2 || time <= times[0] {
        return first;
    }
    let Some(last) = values.last().copied() else {
        return first;
    };
    if time >= times[times.len() - 1] {
        return last;
    }
    let (previous, next, fraction) = locate(times, time);
    match interpolation {
        Interpolation::Step => values[previous],
        Interpolation::Linear => values[previous].lerp(values[next], fraction),
    }
}

fn sample_weights(
    times: &[f32],
    values: &[Vec<f32>],
    interpolation: Interpolation,
    time: f32,
) -> Vec<f32> {
    let Some(first) = values.first() else {
        return Vec::new();
    };
    if times.len() < 2 || time <= times[0] {
        return first.clone();
    }
    let Some(last) = values.last() else {
        return first.clone();
    };
    if time >= times[times.len() - 1] {
        return last.clone();
    }
    let (previous, next, fraction) = locate(times, time);
    match interpolation {
        Interpolation::Step => values[previous].clone(),
        Interpolation::Linear => values[previous]
            .iter()
            .zip(&values[next])
            .map(|(from, to)| from + (to - from) * fraction)
            .collect(),
    }
}

fn sample_rotations(
    times: &[f32],
    values: &[Quat],
    interpolation: Interpolation,
    time: f32,
) -> Quat {
    let Some(first) = values.first().copied() else {
        return Quat::IDENTITY;
    };
    if times.len() < 2 || time <= times[0] {
        return first;
    }
    let Some(last) = values.last().copied() else {
        return first;
    };
    if time >= times[times.len() - 1] {
        return last;
    }
    let (previous, next, fraction) = locate(times, time);
    match interpolation {
        Interpolation::Step => values[previous],
        Interpolation::Linear => values[previous].slerp(values[next], fraction),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::Quat;

    fn clip() -> AnimationClip {
        AnimationClip {
            name: "test".to_string(),
            duration: 2.0,
            tracks: vec![
                AnimationTrack {
                    node: 1,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 1.0, 2.0],
                    values: TrackValues::Translations(vec![
                        Vec3::new(0.0, 0.0, 0.0),
                        Vec3::new(0.0, 2.0, 0.0),
                        Vec3::new(0.0, 4.0, 0.0),
                    ]),
                },
                AnimationTrack {
                    node: 1,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 2.0],
                    values: TrackValues::Rotations(vec![
                        Quat::IDENTITY,
                        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                    ]),
                },
                AnimationTrack {
                    node: 2,
                    interpolation: Interpolation::Step,
                    times: vec![0.0, 1.0],
                    values: TrackValues::Scales(vec![Vec3::ONE, Vec3::splat(2.0)]),
                },
            ],
        }
    }

    #[test]
    fn samples_linear_translation() {
        let poses = clip().sample(0.5);
        let pose = poses.iter().find(|pose| pose.node == 1).unwrap();
        assert!((pose.translation.unwrap() - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn interpolates_rotation() {
        let poses = clip().sample(2.0);
        let pose = poses.iter().find(|pose| pose.node == 1).unwrap();
        let rotated = pose.rotation.unwrap() * Vec3::X;
        assert!((rotated.x - 0.0).abs() < 1e-4);
        assert!((rotated.z + 1.0).abs() < 1e-4);
    }

    #[test]
    fn step_interpolation_holds_values() {
        let poses = clip().sample(1.5);
        let pose = poses.iter().find(|pose| pose.node == 2).unwrap();
        assert!((pose.scale.unwrap() - Vec3::splat(2.0)).length() < 1e-5);
    }

    #[test]
    fn interpolates_morph_weights() {
        let clip = AnimationClip {
            name: "morph".to_string(),
            duration: 1.0,
            tracks: vec![AnimationTrack {
                node: 0,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Weights(vec![vec![0.0, 1.0], vec![1.0, 0.0]]),
            }],
        };
        let pose = clip.sample(0.25);
        let weights = pose[0].weights.as_ref().unwrap();
        assert!((weights[0] - 0.25).abs() < 1e-5);
        assert!((weights[1] - 0.75).abs() < 1e-5);
    }

    #[test]
    fn clamps_outside_range() {
        let poses = clip().sample(10.0);
        let pose = poses.iter().find(|pose| pose.node == 1).unwrap();
        assert!((pose.translation.unwrap() - Vec3::new(0.0, 4.0, 0.0)).length() < 1e-5);
    }
}
