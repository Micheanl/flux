@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

fn tap(uv: vec2<f32>, offset: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(source, source_sampler, uv + offset, 0.0).rgb;
}

@fragment
fn fs_prefilter(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let dimensions = vec2<f32>(textureDimensions(source));
    let uv = position.xy / dimensions;
    let color = textureSampleLevel(source, source_sampler, uv, 0.0).rgb;
    let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    let knee = max(luma - 1.0, 0.0);
    let weight = knee / max(luma, 1e-4);
    return vec4<f32>(color * weight, 1.0);
}

@fragment
fn fs_downsample(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let dimensions = vec2<f32>(textureDimensions(source));
    let uv = position.xy / dimensions;
    let texel = vec2<f32>(1.0) / dimensions;
    var color = tap(uv, vec2<f32>(0.0)) * 0.125;
    color += (tap(uv, texel * vec2<f32>(-2.0, 2.0))
        + tap(uv, texel * vec2<f32>(2.0, 2.0))
        + tap(uv, texel * vec2<f32>(-2.0, -2.0))
        + tap(uv, texel * vec2<f32>(2.0, -2.0))) * 0.03125;
    color += (tap(uv, texel * vec2<f32>(0.0, 2.0))
        + tap(uv, texel * vec2<f32>(-2.0, 0.0))
        + tap(uv, texel * vec2<f32>(2.0, 0.0))
        + tap(uv, texel * vec2<f32>(0.0, -2.0))) * 0.0625;
    color += (tap(uv, texel * vec2<f32>(-1.0, 1.0))
        + tap(uv, texel * vec2<f32>(1.0, 1.0))
        + tap(uv, texel * vec2<f32>(-1.0, -1.0))
        + tap(uv, texel * vec2<f32>(1.0, -1.0))) * 0.125;
    return vec4<f32>(color, 1.0);
}

@fragment
fn fs_upsample(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let dimensions = vec2<f32>(textureDimensions(source));
    let uv = position.xy / dimensions;
    let texel = vec2<f32>(1.0) / dimensions;
    var color = tap(uv, vec2<f32>(0.0)) * 4.0;
    color += (tap(uv, texel * vec2<f32>(1.0, 0.0))
        + tap(uv, texel * vec2<f32>(-1.0, 0.0))
        + tap(uv, texel * vec2<f32>(0.0, 1.0))
        + tap(uv, texel * vec2<f32>(0.0, -1.0))) * 2.0;
    color += tap(uv, texel * vec2<f32>(1.0, 1.0))
        + tap(uv, texel * vec2<f32>(-1.0, 1.0))
        + tap(uv, texel * vec2<f32>(1.0, -1.0))
        + tap(uv, texel * vec2<f32>(-1.0, -1.0));
    return vec4<f32>(color / 16.0, 1.0);
}
