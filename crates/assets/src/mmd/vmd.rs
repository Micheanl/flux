use foundation::{Quat, Vec3};

use crate::error::AssetError;

const HEADER: &[u8] = b"Vocaloid Motion Data 0002";
const BONE_NAME_SIZE: usize = 15;
const MODEL_NAME_SIZE: usize = 20;

#[derive(Clone, Debug)]
pub struct VmdBoneFrame {
    pub frame: u32,
    pub translation: Vec3,
    pub rotation: Quat,
}

#[derive(Clone, Debug)]
pub struct VmdBoneTrack {
    pub name: String,
    pub frames: Vec<VmdBoneFrame>,
}

#[derive(Clone, Debug, Default)]
pub struct VmdMotion {
    pub bones: Vec<VmdBoneTrack>,
}

struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, count: usize) -> Result<&'a [u8], AssetError> {
        let end = self.offset + count;
        let slice = self
            .data
            .get(self.offset..end)
            .ok_or_else(|| AssetError::invalid("unexpected end of VMD data"))?;
        self.offset = end;
        Ok(slice)
    }

    fn u32(&mut self) -> Result<u32, AssetError> {
        let bytes = self.bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn f32(&mut self) -> Result<f32, AssetError> {
        Ok(f32::from_bits(self.u32()?))
    }

    fn vec3(&mut self) -> Result<Vec3, AssetError> {
        Ok(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }

    fn name(&mut self, size: usize) -> Result<String, AssetError> {
        let bytes = self.bytes(size)?;
        let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(size);
        Ok(decode_shift_jis(&bytes[..end]))
    }
}

fn decode_shift_jis(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    let (text, _, _) = encoding_rs::SHIFT_JIS.decode(bytes);
    text.into_owned()
}

pub fn load_vmd(path: &std::path::Path) -> Result<VmdMotion, AssetError> {
    let data = std::fs::read(path)?;
    parse_vmd(&data)
}

pub fn parse_vmd(data: &[u8]) -> Result<VmdMotion, AssetError> {
    let mut reader = Reader { data, offset: 0 };
    let header = reader.bytes(HEADER.len())?;
    if !header.starts_with(HEADER) {
        return Err(AssetError::invalid("not a VMD file"));
    }
    reader.bytes(30 - HEADER.len())?;
    reader.bytes(MODEL_NAME_SIZE)?;

    let bone_count = reader.u32()?;
    let mut bones: Vec<VmdBoneTrack> = Vec::new();
    for _ in 0..bone_count {
        let name = reader.name(BONE_NAME_SIZE)?;
        let frame = reader.u32()?;
        let translation = reader.vec3()?;
        let rotation = Quat::from_xyzw(reader.f32()?, reader.f32()?, reader.f32()?, reader.f32()?);
        reader.bytes(64)?;
        let track = match bones.iter_mut().find(|track| track.name == name) {
            Some(track) => track,
            None => {
                bones.push(VmdBoneTrack {
                    name,
                    frames: Vec::new(),
                });
                bones.last_mut().expect("just pushed")
            }
        };
        track.frames.push(VmdBoneFrame {
            frame,
            translation,
            rotation,
        });
    }

    for track in &mut bones {
        track.frames.sort_by_key(|frame| frame.frame);
    }
    Ok(VmdMotion { bones })
}
