use std::path::Path;

use renderer::TextureData;

use crate::error::AssetError;

pub fn load_image(path: &Path) -> Result<TextureData, AssetError> {
    let image = image::open(path)?.to_rgba8();
    let (width, height) = image.dimensions();
    Ok(TextureData::from_rgba8(width, height, image.into_raw()))
}
