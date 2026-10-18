@group(0) @binding(0) var<uniform> frame: FrameUniform;
@group(1) @binding(0) var hdr: texture_2d<f32>;
@group(1) @binding(1) var bloom_texture: texture_2d<f32>;
@group(1) @binding(2) var linear_sampler: sampler;
@group(1) @binding(3) var depth_texture: texture_depth_2d;

fn view_depth(coord: vec2<i32>, limit: vec2<i32>) -> f32 {
    let pixel = clamp(coord, vec2<i32>(0), limit);
    let raw = textureLoad(depth_texture, pixel, 0);
    return frame.params.x / max(raw, 1e-6);
}

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

    if (frame.params.y > 0.5) {
        let pixel = vec2<i32>(position.xy);
        let limit = vec2<i32>(frame.viewport.xy) - vec2<i32>(1);
        let center = view_depth(pixel, limit);
        let dx = abs(view_depth(pixel + vec2<i32>(1, 0), limit) - center)
            + abs(view_depth(pixel - vec2<i32>(1, 0), limit) - center);
        let dy = abs(view_depth(pixel + vec2<i32>(0, 1), limit) - center)
            + abs(view_depth(pixel - vec2<i32>(0, 1), limit) - center);
        let edge = smoothstep(0.03, 0.12, (dx + dy) / max(center, 0.01));
        color = mix(color, vec3<f32>(0.05, 0.05, 0.07), edge * 0.85);
    }

    let centered = uv * 2.0 - 1.0;
    color *= 1.0 - dot(centered, centered) * 0.08;
    return vec4<f32>(color, 1.0);
}
