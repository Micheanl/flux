@group(0) @binding(0) var<uniform> view: ViewUniform;
@group(2) @binding(0) var<storage, read> instances: array<Instance>;

@vertex
fn vs_depth(
    @location(0) position: vec3<f32>,
    @builtin(instance_index) instance_index: u32,
) -> @builtin(position) vec4<f32> {
    let model = instances[instance_index].model;
    return view.view_proj * model * vec4<f32>(position, 1.0);
}
