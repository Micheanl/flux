@group(0) @binding(0) var<uniform> frame: FrameUniform;
@group(1) @binding(0) var hdr: texture_2d<f32>;
@group(1) @binding(1) var bloom_texture: texture_2d<f32>;
@group(1) @binding(2) var linear_sampler: sampler;

fn aces(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((color * (a * color + b)) / (color * (c * color + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_tonemap(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = position.xy / frame.viewport.xy;
    var color = textureSampleLevel(hdr, linear_sampler, uv, 0.0).rgb;
    let bloom = textureSampleLevel(bloom_texture, linear_sampler, uv, 0.0).rgb;
    color = color * frame.exposure.x + bloom * frame.exposure.y;
    color = aces(color);

    let centered = uv * 2.0 - 1.0;
    color *= 1.0 - dot(centered, centered) * 0.08;
    return vec4<f32>(color, 1.0);
}
