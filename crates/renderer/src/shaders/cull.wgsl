@group(0) @binding(0) var<uniform> view: ViewUniform;
@group(1) @binding(0) var<storage, read> instances: array<Instance>;
@group(1) @binding(1) var<storage, read> meshes: array<Mesh>;
@group(2) @binding(0) var<storage, read_write> draw_commands: array<DrawArgs>;
@group(2) @binding(1) var<storage, read_write> visible_count: atomic<u32>;

fn aabb_visible(bounds_min: vec3<f32>, bounds_max: vec3<f32>) -> bool {
    let center = (bounds_min + bounds_max) * 0.5;
    let extents = (bounds_max - bounds_min) * 0.5;
    for (var i = 0; i < 6; i = i + 1) {
        let plane = view.planes[i];
        let distance = dot(plane.xyz, center) + plane.w;
        let radius = dot(abs(plane.xyz), extents);
        if (distance + radius < 0.0) {
            return false;
        }
    }
    return true;
}

@compute @workgroup_size(64)
fn cull_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let index = id.x;
    if (index >= view.counts.x) {
        return;
    }
    let instance = instances[index];
    if (!aabb_visible(instance.bounds_min.xyz, instance.bounds_max.xyz)) {
        return;
    }

    let mesh = meshes[instance.mesh];
    let slot = atomicAdd(&visible_count, 1u);
    var command: DrawArgs;
    command.index_count = mesh.index_count;
    command.instance_count = 1u;
    command.first_index = mesh.index_offset;
    command.base_vertex = mesh.base_vertex;
    command.first_instance = index;
    draw_commands[slot] = command;
}
