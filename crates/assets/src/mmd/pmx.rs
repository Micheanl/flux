use foundation::{Vec2, Vec3, Vec4};

use crate::error::AssetError;

const HEADER: &[u8; 4] = b"PMX ";

#[derive(Clone, Debug)]
pub struct PmxVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct PmxMaterial {
    pub diffuse: Vec4,
    pub double_sided: bool,
    pub texture: i32,
    pub surface_count: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct PmxInherit {
    pub source: i32,
    pub ratio: f32,
    pub rotation: bool,
    pub translation: bool,
}

#[derive(Clone, Debug)]
pub struct PmxBone {
    pub name: String,
    pub position: Vec3,
    pub parent: i32,
    pub inherit: Option<PmxInherit>,
}

#[derive(Clone, Debug)]
pub struct PmxModel {
    pub name: String,
    pub vertices: Vec<PmxVertex>,
    pub indices: Vec<u32>,
    pub textures: Vec<String>,
    pub materials: Vec<PmxMaterial>,
    pub bones: Vec<PmxBone>,
}

#[derive(Clone, Copy)]
enum TextEncoding {
    Utf16,
    Utf8,
}

struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
    text: TextEncoding,
    vertex_index_size: usize,
    texture_index_size: usize,
    material_index_size: usize,
    bone_index_size: usize,
    morph_index_size: usize,
    rigid_body_index_size: usize,
    additional_vec4: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Result<Self, AssetError> {
        let mut reader = Self {
            data,
            offset: 0,
            text: TextEncoding::Utf16,
            vertex_index_size: 0,
            texture_index_size: 0,
            material_index_size: 0,
            bone_index_size: 0,
            morph_index_size: 0,
            rigid_body_index_size: 0,
            additional_vec4: 0,
        };
        reader.expect(HEADER)?;
        let version = reader.f32()?;
        if !(1.9..3.0).contains(&version) {
            return Err(AssetError::invalid("unsupported PMX version"));
        }
        let globals = reader.u8()? as usize;
        if globals < 8 {
            return Err(AssetError::invalid("truncated PMX header"));
        }
        let mut settings = Vec::with_capacity(globals);
        for _ in 0..globals {
            settings.push(reader.u8()?);
        }
        reader.text = match settings[0] {
            0 => TextEncoding::Utf16,
            1 => TextEncoding::Utf8,
            _ => return Err(AssetError::invalid("unknown PMX text encoding")),
        };
        reader.additional_vec4 = settings[1] as usize;
        reader.vertex_index_size = settings[2] as usize;
        reader.texture_index_size = settings[3] as usize;
        reader.material_index_size = settings[4] as usize;
        reader.bone_index_size = settings[5] as usize;
        reader.morph_index_size = settings[6] as usize;
        reader.rigid_body_index_size = settings[7] as usize;
        Ok(reader)
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.offset)
    }

    fn bytes(&mut self, count: usize) -> Result<&'a [u8], AssetError> {
        let end = self.offset + count;
        let slice = self
            .data
            .get(self.offset..end)
            .ok_or_else(|| AssetError::invalid("unexpected end of PMX data"))?;
        self.offset = end;
        Ok(slice)
    }

    fn expect(&mut self, expected: &[u8]) -> Result<(), AssetError> {
        if self.bytes(expected.len())? == expected {
            Ok(())
        } else {
            Err(AssetError::invalid("not a PMX file"))
        }
    }

    fn u8(&mut self) -> Result<u8, AssetError> {
        Ok(self.bytes(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, AssetError> {
        let bytes = self.bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn i32(&mut self) -> Result<i32, AssetError> {
        let bytes = self.bytes(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u32(&mut self) -> Result<u32, AssetError> {
        Ok(self.i32()? as u32)
    }

    fn f32(&mut self) -> Result<f32, AssetError> {
        Ok(f32::from_bits(self.u32()?))
    }

    fn vec2(&mut self) -> Result<Vec2, AssetError> {
        Ok(Vec2::new(self.f32()?, self.f32()?))
    }

    fn vec3(&mut self) -> Result<Vec3, AssetError> {
        Ok(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }

    fn vec4(&mut self) -> Result<Vec4, AssetError> {
        Ok(Vec4::new(
            self.f32()?,
            self.f32()?,
            self.f32()?,
            self.f32()?,
        ))
    }

    fn index(&mut self, size: usize) -> Result<i32, AssetError> {
        match size {
            1 => Ok(self.u8()? as i8 as i32),
            2 => Ok(self.u16()? as i16 as i32),
            4 => self.i32(),
            _ => Err(AssetError::invalid("unsupported PMX index size")),
        }
    }

    fn vertex_index(&mut self) -> Result<u32, AssetError> {
        match self.vertex_index_size {
            1 => Ok(self.u8()? as u32),
            2 => Ok(self.u16()? as u32),
            4 => self.u32(),
            _ => Err(AssetError::invalid("unsupported PMX vertex index size")),
        }
    }

    fn texture_index(&mut self) -> Result<i32, AssetError> {
        let size = self.texture_index_size;
        self.index(size)
    }

    fn material_index(&mut self) -> Result<i32, AssetError> {
        let size = self.material_index_size;
        self.index(size)
    }

    fn bone_index(&mut self) -> Result<i32, AssetError> {
        let size = self.bone_index_size;
        self.index(size)
    }

    fn morph_index(&mut self) -> Result<i32, AssetError> {
        let size = self.morph_index_size;
        self.index(size)
    }

    fn rigid_body_index(&mut self) -> Result<i32, AssetError> {
        let size = self.rigid_body_index_size;
        self.index(size)
    }

    fn text(&mut self) -> Result<String, AssetError> {
        let length = self.i32()?;
        if length < 0 {
            return Err(AssetError::invalid("negative PMX text length"));
        }
        let bytes = self.bytes(length as usize)?;
        Ok(match self.text {
            TextEncoding::Utf16 => String::from_utf16_lossy(
                &bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_le_bytes(*pair))
                    .collect::<Vec<u16>>(),
            ),
            TextEncoding::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
        })
    }

    fn vertices(&mut self) -> Result<Vec<PmxVertex>, AssetError> {
        let count = self.i32()?;
        if count < 0 || count as usize > self.remaining() / 20 {
            return Err(AssetError::invalid("invalid PMX vertex count"));
        }
        let mut vertices = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let position = self.vec3()?;
            let normal = self.vec3()?;
            let uv = self.vec2()?;
            for _ in 0..self.additional_vec4 {
                self.vec4()?;
            }
            let weight_type = self.u8()?;
            let mut joints = [0u16; 4];
            let mut weights = [0.0f32; 4];
            match weight_type {
                0 => {
                    joints[0] = self.bone_index()?.max(0) as u16;
                    weights[0] = 1.0;
                }
                1 => {
                    joints[0] = self.bone_index()?.max(0) as u16;
                    joints[1] = self.bone_index()?.max(0) as u16;
                    let weight = self.f32()?;
                    weights[0] = weight;
                    weights[1] = 1.0 - weight;
                }
                2 | 4 => {
                    for joint in &mut joints {
                        *joint = self.bone_index()?.max(0) as u16;
                    }
                    for weight in &mut weights {
                        *weight = self.f32()?;
                    }
                }
                3 => {
                    joints[0] = self.bone_index()?.max(0) as u16;
                    joints[1] = self.bone_index()?.max(0) as u16;
                    let weight = self.f32()?;
                    weights[0] = weight;
                    weights[1] = 1.0 - weight;
                    self.vec3()?;
                    self.vec3()?;
                    self.vec3()?;
                }
                _ => return Err(AssetError::invalid("unknown PMX weight type")),
            }
            self.f32()?;
            vertices.push(PmxVertex {
                position,
                normal,
                uv,
                joints,
                weights,
            });
        }
        Ok(vertices)
    }

    fn faces(&mut self) -> Result<Vec<u32>, AssetError> {
        let count = self.i32()?;
        if count < 0 {
            return Err(AssetError::invalid("negative PMX face count"));
        }
        let mut indices = Vec::with_capacity(count as usize);
        for _ in 0..count {
            indices.push(self.vertex_index()?);
        }
        Ok(indices)
    }

    fn textures(&mut self) -> Result<Vec<String>, AssetError> {
        let count = self.i32()?;
        if count < 0 {
            return Err(AssetError::invalid("negative PMX texture count"));
        }
        let mut textures = Vec::with_capacity(count as usize);
        for _ in 0..count {
            textures.push(self.text()?);
        }
        Ok(textures)
    }

    fn materials(&mut self) -> Result<Vec<PmxMaterial>, AssetError> {
        let count = self.i32()?;
        if count < 0 {
            return Err(AssetError::invalid("negative PMX material count"));
        }
        let mut materials = Vec::with_capacity(count as usize);
        for _ in 0..count {
            self.text()?;
            self.text()?;
            let diffuse = self.vec4()?;
            self.vec3()?;
            self.f32()?;
            self.vec3()?;
            let flags = self.u8()?;
            self.vec4()?;
            self.f32()?;
            let texture = self.texture_index()?;
            self.texture_index()?;
            self.u8()?;
            let toon_reference = self.u8()?;
            if toon_reference == 0 {
                self.texture_index()?;
            } else {
                self.u8()?;
            }
            self.text()?;
            let surface_count = self.u32()?;
            materials.push(PmxMaterial {
                diffuse,
                double_sided: flags & 0x01 != 0,
                texture,
                surface_count,
            });
        }
        Ok(materials)
    }

    fn bones(&mut self) -> Result<Vec<PmxBone>, AssetError> {
        let count = self.i32()?;
        if count < 0 {
            return Err(AssetError::invalid("negative PMX bone count"));
        }
        let mut bones = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let name = self.text()?;
            self.text()?;
            let position = self.vec3()?;
            let parent = self.bone_index()?;
            self.i32()?;
            let flags = self.u16()?;
            let mut inherit = None;
            if flags & 0x0001 != 0 {
                self.bone_index()?;
            } else {
                self.vec3()?;
            }
            if flags & 0x0100 != 0 || flags & 0x0200 != 0 {
                let source = self.bone_index()?;
                let ratio = self.f32()?;
                inherit = Some(PmxInherit {
                    source,
                    ratio,
                    rotation: flags & 0x0100 != 0,
                    translation: flags & 0x0200 != 0,
                });
            }
            if flags & 0x0400 != 0 {
                self.vec3()?;
            }
            if flags & 0x0800 != 0 {
                self.vec3()?;
                self.vec3()?;
            }
            if flags & 0x2000 != 0 {
                self.i32()?;
            }
            if flags & 0x0020 != 0 {
                self.bone_index()?;
                let loops = self.i32()?;
                if loops < 0 {
                    return Err(AssetError::invalid("negative PMX IK loop count"));
                }
                self.f32()?;
                let links = self.i32()?;
                if links < 0 {
                    return Err(AssetError::invalid("negative PMX IK link count"));
                }
                for _ in 0..links {
                    self.bone_index()?;
                    if self.u8()? != 0 {
                        self.vec3()?;
                        self.vec3()?;
                    }
                }
            }
            bones.push(PmxBone {
                name,
                position,
                parent,
                inherit,
            });
        }
        Ok(bones)
    }

    fn morphs(&mut self) -> Result<(), AssetError> {
        let count = self.i32()?;
        if count < 0 {
            return Err(AssetError::invalid("negative PMX morph count"));
        }
        for _ in 0..count {
            self.text()?;
            self.text()?;
            self.u8()?;
            let kind = self.u8()?;
            let offsets = self.i32()?;
            if offsets < 0 {
                return Err(AssetError::invalid("negative PMX morph offset count"));
            }
            for _ in 0..offsets {
                match kind {
                    0 | 9 => {
                        self.morph_index()?;
                        self.f32()?;
                    }
                    1 => {
                        self.vertex_index()?;
                        self.vec3()?;
                    }
                    2 => {
                        self.bone_index()?;
                        self.vec3()?;
                        self.vec4()?;
                    }
                    3..=7 => {
                        self.vertex_index()?;
                        self.vec4()?;
                    }
                    8 => {
                        self.material_index()?;
                        self.u8()?;
                        self.vec4()?;
                        self.vec3()?;
                        self.f32()?;
                        self.vec3()?;
                        self.vec4()?;
                        self.f32()?;
                        self.vec4()?;
                        self.vec4()?;
                        self.vec4()?;
                    }
                    10 => {
                        self.rigid_body_index()?;
                        self.u8()?;
                        self.vec3()?;
                        self.vec3()?;
                    }
                    _ => return Err(AssetError::invalid("unknown PMX morph type")),
                }
            }
        }
        Ok(())
    }
}

pub fn load_pmx(path: &std::path::Path) -> Result<PmxModel, AssetError> {
    let data = std::fs::read(path)?;
    parse_pmx(&data)
}

pub fn parse_pmx(data: &[u8]) -> Result<PmxModel, AssetError> {
    let mut reader = Reader::new(data)?;
    let name = reader.text()?;
    reader.text()?;
    reader.text()?;
    reader.text()?;
    let vertices = reader.vertices()?;
    let indices = reader.faces()?;
    let textures = reader.textures()?;
    let materials = reader.materials()?;
    let bones = reader.bones()?;
    reader.morphs()?;
    Ok(PmxModel {
        name,
        vertices,
        indices,
        textures,
        materials,
        bones,
    })
}
