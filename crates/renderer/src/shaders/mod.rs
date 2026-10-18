pub const COMMON: &str = include_str!("common.wgsl");
pub const MESH: &str = include_str!("mesh.wgsl");
pub const DEPTH: &str = include_str!("depth.wgsl");
pub const CULL: &str = include_str!("cull.wgsl");
pub const SKY: &str = include_str!("sky.wgsl");
pub const BLOOM: &str = include_str!("bloom.wgsl");
pub const TONEMAP: &str = include_str!("tonemap.wgsl");

pub fn module(name: &str) -> String {
    let body = match name {
        "mesh" => MESH,
        "depth" => DEPTH,
        "cull" => CULL,
        "sky" => SKY,
        "bloom" => BLOOM,
        "tonemap" => TONEMAP,
        other => panic!("unknown shader {other}"),
    };
    format!("{COMMON}\n{body}")
}
