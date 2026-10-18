@group(0) @binding(0) var<uniform> frame: FrameUniform;
@group(1) @binding(0) var albedo_map: texture_2d_array<f32>;
@group(1) @binding(1) var normal_map: texture_2d_array<f32>;
@group(1) @binding(2) var mr_map: texture_2d_array<f32>;
@group(1) @binding(3) var emissive_map: texture_2d_array<f32>;
@group(1) @binding(4) var linear_sampler: sampler;
@group(1) @binding(5) var shadow_map: texture_depth_2d_array;
@group(1) @binding(6) var shadow_sampler: sampler_comparison;
@group(2) @binding(0) var<storage, read> instances: array<Instance>;
@group(2) @binding(1) var<storage, read> materials: array<Material>;
@group(2) @binding(2) var<storage, read> point_lights: array<PointLight>;
@group(3) @binding(0) var<storage, read> joint_matrices: array<mat4x4<f32>>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
    @location(4) joints: vec4<u32>,
    @location(5) weights: vec4<f32>,
    @builtin(instance_index) instance_index: u32,
};

struct VertexOutput {
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) @interpolate(flat) material_index: u32,
};

fn skin_matrix(instance: Instance, joints: vec4<u32>, weights: vec4<f32>) -> mat4x4<f32> {
    let base = instance.skin_offset;
    return joint_matrices[base + joints.x] * weights.x
        + joint_matrices[base + joints.y] * weights.y
        + joint_matrices[base + joints.z] * weights.z
        + joint_matrices[base + joints.w] * weights.w;
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let instance = instances[input.instance_index];
    var output: VertexOutput;
    if ((instance.flags & 1u) != 0u) {
        let skin = skin_matrix(instance, input.joints, input.weights);
        let world = skin * vec4<f32>(input.position, 1.0);
        output.clip = frame.view_proj * skin * vec4<f32>(input.position, 1.0);
        output.world_pos = world.xyz;
        output.normal = normalize((skin * vec4<f32>(input.normal, 0.0)).xyz);
        output.tangent = vec4<f32>(
            normalize((skin * vec4<f32>(input.tangent.xyz, 0.0)).xyz),
            input.tangent.w,
        );
    } else {
        let world = instance.model * vec4<f32>(input.position, 1.0);
        output.clip = frame.view_proj * instance.model * vec4<f32>(input.position, 1.0);
        output.world_pos = world.xyz;
        output.normal = normalize((instance.model * vec4<f32>(input.normal, 0.0)).xyz);
        output.tangent = vec4<f32>(
            normalize((instance.model * vec4<f32>(input.tangent.xyz, 0.0)).xyz),
            input.tangent.w,
        );
    }
    output.uv = input.uv;
    output.material_index = instance.material;
    return output;
}

fn linearize_depth(depth: f32, near: f32) -> f32 {
    return near / max(depth, 1e-6);
}

fn sample_shadow(world_pos: vec3<f32>, normal: vec3<f32>, view_depth: f32) -> f32 {
    var cascade = 0u;
    if (view_depth > frame.cascade_splits.x) { cascade = 1u; }
    if (view_depth > frame.cascade_splits.y) { cascade = 2u; }
    if (view_depth > frame.cascade_splits.z) { cascade = 3u; }

    let vp = frame.cascade_vp[cascade];

    let offset_world = world_pos + normal * frame.shadow_texel.w * (1.0 + f32(cascade));
    let light_clip = vp * vec4<f32>(offset_world, 1.0);
    let ndc = light_clip.xyz / light_clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return 1.0;
    }

    let bias = frame.shadow_texel.z * (1.0 + f32(cascade) * 0.5);
    let texel = frame.shadow_texel.xy;
    let reference = ndc.z + bias;
    var lit = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel;
            lit += textureSampleCompareLevel(
                shadow_map,
                shadow_sampler,
                uv + offset,
                i32(cascade),
                reference,
            );
        }
    }
    return lit / 9.0;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let material = materials[input.material_index];
    let uv = input.uv * material.roughness_uv_scale.yz + material.uv_offset_flags.xy;

    let albedo_texture = textureSample(albedo_map, linear_sampler, uv, i32(material.textures.x));
    let mr = textureSample(mr_map, linear_sampler, uv, i32(material.textures.z));
    let normal_texture = textureSample(normal_map, linear_sampler, uv, i32(material.textures.y));
    let emissive_texture = textureSample(emissive_map, linear_sampler, uv, i32(material.textures.w));

    let albedo = albedo_texture.rgb * material.base_color.rgb;
    let metallic = clamp(mr.b * material.emissive_metallic.w, 0.0, 1.0);
    let roughness = clamp(mr.g * material.roughness_uv_scale.x, 0.045, 1.0);

    let n = normalize(input.normal);
    let t = normalize(input.tangent.xyz - n * dot(n, input.tangent.xyz));
    let b = cross(n, t) * input.tangent.w;
    let normal_sample = normal_texture.xyz * 2.0 - 1.0;
    let shading_normal = normalize(mat3x3<f32>(t, b, n) * normal_sample);

    let view_dir = normalize(frame.camera_pos.xyz - input.world_pos);
    let view_depth = linearize_depth(input.clip.z, frame.params.x);
    let shadow = sample_shadow(input.world_pos, n, view_depth);
    let sun_color = frame.sun_color.rgb * frame.sun_dir.w;
    let light_dir = normalize(frame.sun_dir.xyz);

    var color = evaluate_brdf(shading_normal, view_dir, light_dir, albedo, metallic, roughness)
        * sun_color
        * shadow;

    let ambient_scale = frame.exposure.w;
    let sky_diffuse = sky_color(shading_normal, light_dir, frame.sun_color.rgb, frame.sun_dir.w);
    let reflected = reflect(-view_dir, shading_normal);
    let sky_specular = sky_color(reflected, light_dir, frame.sun_color.rgb, frame.sun_dir.w);
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    let ambient_fresnel = fresnel_schlick(max(dot(view_dir, shading_normal), 0.0), f0);
    color += sky_diffuse * albedo * (1.0 - metallic) * ambient_scale;
    color += sky_specular * ambient_fresnel * (1.0 - roughness * 0.6) * ambient_scale;
    color += material.emissive_metallic.rgb * emissive_texture.rgb;

    let point_count = u32(frame.params.w);
    for (var index = 0u; index < point_count; index = index + 1u) {
        let light = point_lights[index];
        let to_light = light.position.xyz - input.world_pos;
        let distance = length(to_light);
        let to_light_dir = to_light / max(distance, 1e-4);
        if (dot(shading_normal, to_light_dir) > 0.0) {
            let attenuation = point_light_attenuation(distance, light.position.w);
            color += evaluate_brdf(
                shading_normal,
                view_dir,
                to_light_dir,
                albedo,
                metallic,
                roughness,
            ) * light.color.rgb * attenuation;
        }
    }

    return vec4<f32>(color, 1.0);
}
