#[derive(Debug)]
pub enum AssetError {
    Gltf(gltf::Error),
    Image(image::ImageError),
    Io(std::io::Error),
    Unsupported(String),
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetError::Gltf(error) => write!(f, "glTF parse error: {error}"),
            AssetError::Image(error) => write!(f, "image decode error: {error}"),
            AssetError::Io(error) => write!(f, "io error: {error}"),
            AssetError::Unsupported(message) => write!(f, "unsupported asset: {message}"),
        }
    }
}

impl std::error::Error for AssetError {}

impl AssetError {
    pub(crate) fn invalid(message: &str) -> Self {
        AssetError::Unsupported(message.to_string())
    }
}

impl From<gltf::Error> for AssetError {
    fn from(error: gltf::Error) -> Self {
        AssetError::Gltf(error)
    }
}

impl From<image::ImageError> for AssetError {
    fn from(error: image::ImageError) -> Self {
        AssetError::Image(error)
    }
}

impl From<std::io::Error> for AssetError {
    fn from(error: std::io::Error) -> Self {
        AssetError::Io(error)
    }
}
