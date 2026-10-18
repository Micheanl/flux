@group(0) @binding(0) var<uniform> frame: FrameUniform;

fn sky_radiance(direction: vec3<f32>) -> vec3<f32> {
    return sky_color(
        direction,
        normalize(frame.sun_dir.xyz),
        frame.sun_color.rgb,
        frame.sun_dir.w,
    );
}

@fragment
fn fs_sky(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = position.xy / frame.viewport.xy;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let near = frame.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    let direction = normalize(near.xyz / near.w - frame.camera_pos.xyz);
    return vec4<f32>(sky_radiance(direction), 1.0);
}
