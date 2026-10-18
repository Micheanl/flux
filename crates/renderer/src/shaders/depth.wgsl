@group(0) @binding(0) var<uniform> view: ViewUniform;
@group(2) @binding(0) var<storage, read> instances: array<Instance>;
@group(3) @binding(0) var<storage, read> joint_matrices: array<mat4x4<f32>>;

fn skin_matrix(instance: Instance, joints: vec4<u32>, weights: vec4<f32>) -> mat4x4<f32> {
    let base = instance.skin_offset;
    return joint_matrices[base + joints.x] * weights.x
        + joint_matrices[base + joints.y] * weights.y
        + joint_matrices[base + joints.z] * weights.z
        + joint_matrices[base + joints.w] * weights.w;
}

@vertex
fn vs_depth(
    @location(0) position: vec3<f32>,
    @location(4) joints: vec4<u32>,
    @location(5) weights: vec4<f32>,
    @builtin(instance_index) instance_index: u32,
) -> @builtin(position) @invariant vec4<f32> {
    let instance = instances[instance_index];
    if ((instance.flags & 1u) != 0u) {
        let skin = skin_matrix(instance, joints, weights);
        return view.view_proj * skin * vec4<f32>(position, 1.0);
    }
    return view.view_proj * instance.model * vec4<f32>(position, 1.0);
}
