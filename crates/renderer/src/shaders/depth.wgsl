@group(0) @binding(0) var<uniform> view: ViewUniform;
@group(1) @binding(0) var albedo_map: texture_2d_array<f32>;
@group(1) @binding(4) var linear_sampler: sampler;
@group(2) @binding(0) var<storage, read> instances: array<Instance>;
@group(2) @binding(1) var<storage, read> materials: array<Material>;
@group(3) @binding(0) var<storage, read> joint_matrices: array<mat4x4<f32>>;

struct DepthInput {
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(4) joints: vec4<u32>,
    @location(5) weights: vec4<f32>,
    @builtin(instance_index) instance_index: u32,
};

struct DepthOutput {
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) material: u32,
};

fn skin_matrix(instance: Instance, joints: vec4<u32>, weights: vec4<f32>) -> mat4x4<f32> {
    let base = instance.skin_offset;
    return joint_matrices[base + joints.x] * weights.x
        + joint_matrices[base + joints.y] * weights.y
        + joint_matrices[base + joints.z] * weights.z
        + joint_matrices[base + joints.w] * weights.w;
}

@vertex
fn vs_depth(input: DepthInput) -> DepthOutput {
    let instance = instances[input.instance_index];
    var output: DepthOutput;
    if ((instance.flags & 1u) != 0u) {
        let skin = skin_matrix(instance, input.joints, input.weights);
        output.clip = view.view_proj * instance.model * skin * vec4<f32>(input.position, 1.0);
    } else {
        output.clip = view.view_proj * instance.model * vec4<f32>(input.position, 1.0);
    }
    output.uv = input.uv;
    output.material = instance.material;
    return output;
}

@fragment
fn fs_depth(input: DepthOutput, @builtin(front_facing) front_facing: bool) {
    let material = materials[input.material];
    let alpha_mode = material.flags.z;
    if (alpha_mode > 1.5) {
        discard;
    }
    if (alpha_mode > 0.5 && alpha_mode < 1.5) {
        let uv = input.uv * material.roughness_uv_scale.yz + material.uv_offset_flags.xy;
        let alpha =
            textureSample(albedo_map, linear_sampler, uv, i32(material.textures.x)).a
            * material.base_color.a;
        if (alpha < material.flags.w) {
            discard;
        }
    }
    if (!front_facing && material.flags.x < 0.5) {
        discard;
    }
}
