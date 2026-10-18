const PI: f32 = 3.141592653589793;
const CASCADE_COUNT: u32 = 4u;

struct ViewUniform {
    view_proj: mat4x4<f32>,
    planes: array<vec4<f32>, 6>,
    counts: vec4<u32>,
    _pad: array<vec4<f32>, 5>,
};

struct FrameUniform {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    cascade_splits: vec4<f32>,
    shadow_texel: vec4<f32>,
    cascade_vp: array<mat4x4<f32>, CASCADE_COUNT>,
    exposure: vec4<f32>,
    params: vec4<f32>,
    viewport: vec4<f32>,
};

struct Instance {
    model: mat4x4<f32>,
    bounds_min: vec4<f32>,
    bounds_max: vec4<f32>,
    mesh: u32,
    material: u32,
    flags: u32,
    skin_offset: u32,
};

struct Mesh {
    index_offset: u32,
    index_count: u32,
    base_vertex: i32,
    _pad: u32,
};

struct Material {
    base_color: vec4<f32>,
    emissive_metallic: vec4<f32>,
    roughness_uv_scale: vec4<f32>,
    uv_offset_flags: vec4<f32>,
    textures: vec4<u32>,
    flags: vec4<f32>,
};

struct PunctualLight {
    position: vec4<f32>,
    color: vec4<f32>,
    direction: vec4<f32>,
    cone: vec4<f32>,
};

struct DrawArgs {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
};

fn saturate(value: f32) -> f32 {
    return clamp(value, 0.0, 1.0);
}

fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denominator = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / max(PI * denominator * denominator, 1e-5);
}

fn geometry_schlick(n_dot_x: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_x / max(n_dot_x * (1.0 - k) + k, 1e-5);
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(saturate(1.0 - cos_theta), 5.0);
}

fn evaluate_brdf(
    n: vec3<f32>,
    v: vec3<f32>,
    l: vec3<f32>,
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
) -> vec3<f32> {
    let h = normalize(v + l);
    let n_dot_l = max(dot(n, l), 0.0);
    let n_dot_v = max(dot(n, v), 1e-4);
    let n_dot_h = max(dot(n, h), 0.0);
    let v_dot_h = max(dot(v, h), 0.0);

    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    let f = fresnel_schlick(v_dot_h, f0);
    let d = distribution_ggx(n_dot_h, roughness);
    let g = geometry_schlick(n_dot_v, roughness) * geometry_schlick(n_dot_l, roughness);
    let specular = (d * g * f) / max(4.0 * n_dot_v * n_dot_l, 1e-4);
    let diffuse = (vec3<f32>(1.0) - f) * (1.0 - metallic) * albedo / PI;
    return (diffuse + specular) * n_dot_l;
}

fn point_light_attenuation(distance: f32, radius: f32) -> f32 {
    let d = max(distance, 1e-4);
    let falloff = saturate(1.0 - pow(d / max(radius, 1e-3), 4.0));
    return falloff * falloff / (d * d + 1.0);
}

fn sky_color(direction: vec3<f32>, sun_dir: vec3<f32>, sun_color: vec3<f32>, intensity: f32) -> vec3<f32> {
    let up = max(direction.y, 0.0);
    let horizon = vec3<f32>(0.42, 0.58, 0.85);
    let zenith = vec3<f32>(0.06, 0.14, 0.38);
    var color = mix(horizon, zenith, pow(up, 0.4));
    color = mix(color, vec3<f32>(0.28, 0.32, 0.4), pow(1.0 - up, 10.0));

    let sun = max(dot(direction, sun_dir), 0.0);
    color += sun_color * intensity * pow(sun, 6.0) * 0.02;
    color += sun_color * intensity * pow(sun, 1200.0) * 8.0;

    let ground = mix(vec3<f32>(0.09, 0.08, 0.07), vec3<f32>(0.03, 0.03, 0.035), saturate(-direction.y));
    return mix(color, ground, smoothstep(0.0, -0.06, direction.y));
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(positions[index], 0.0, 1.0);
}
