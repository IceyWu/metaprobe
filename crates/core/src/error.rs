use std::fmt;

#[derive(Debug)]
pub enum MetaprobeError {
    FileNotFound(String),
    UnsupportedFormat(String),
    DecodeFailed(String),
    IoError(std::io::Error),
}

impl fmt::Display for MetaprobeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetaprobeError::FileNotFound(path) => write!(f, "File not found: {}", path),
            MetaprobeError::UnsupportedFormat(ext) => write!(f, "Unsupported format: {}", ext),
            MetaprobeError::DecodeFailed(path) => write!(f, "Failed to decode: {}", path),
            MetaprobeError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for MetaprobeError {}

impl From<std::io::Error> for MetaprobeError {
    fn from(e: std::io::Error) -> Self {
        MetaprobeError::IoError(e)
    }
}
