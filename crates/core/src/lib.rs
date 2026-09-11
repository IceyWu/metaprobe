pub mod color_space;
pub mod dimensions;
pub mod error;
pub mod exif_parser;
pub mod format;
pub mod hash;
pub mod video;

use std::collections::HashMap;

use color_space::{detect_color_space, extract_icc_profile, parse_icc_metadata};
use dimensions::get_dimensions;
use error::MetaprobeError;
use exif_parser::parse_exif;
use format::{detect_format, format_from_extension, ImageFormat};
use video::parse_video_with_source_size;

use serde::{Deserialize, Serialize};

/// Unified metadata result for a single image or video.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaMeta {
    pub kind: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub color_space: String,
    pub exif: HashMap<String, String>,
    pub icc: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_creation_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overall_bitrate: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_bitrate: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_rate: Option<f64>,
    pub metadata: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_hash: Option<String>,
}

/// Kept as a Rust-side alias for image-oriented integrations.
pub type ImageMeta = MediaMeta;

/// Extract metadata from raw bytes with a filename hint for extension-based format detection.
pub fn extract_from_bytes(data: &[u8], filename_hint: &str) -> Result<ImageMeta, MetaprobeError> {
    extract_from_bytes_with_options(data, filename_hint, true, None)
}

/// Extract metadata without computing the file hash.
///
/// Browser integrations can compute SHA-256 with the platform crypto API while
/// metadata parsing runs, avoiding a second full-buffer pass in WASM.
pub fn extract_from_bytes_without_hash(
    data: &[u8],
    filename_hint: &str,
) -> Result<ImageMeta, MetaprobeError> {
    extract_from_bytes_with_options(data, filename_hint, false, None)
}

/// Extract metadata without hashing while preserving the original media size.
/// This is used when a caller supplies only a JPEG/ISO-BMFF metadata slice.
pub fn extract_from_bytes_without_hash_with_source_size(
    data: &[u8],
    filename_hint: &str,
    source_size: u64,
) -> Result<ImageMeta, MetaprobeError> {
    extract_from_bytes_with_options(data, filename_hint, false, Some(source_size))
}

fn extract_from_bytes_with_options(
    data: &[u8],
    filename_hint: &str,
    include_hash: bool,
    source_size: Option<u64>,
) -> Result<ImageMeta, MetaprobeError> {
    let ext_format = format_from_extension(filename_hint);

    // Detect actual format by magic bytes (takes precedence)
    let format = detect_format(data).or(ext_format).ok_or_else(|| {
        let ext = filename_hint.rsplit('.').next().unwrap_or("unknown");
        MetaprobeError::UnsupportedFormat(ext.to_string())
    })?;

    extract_from_bytes_with_format_options(data, format, filename_hint, include_hash, source_size)
}

/// Extract metadata when format is already known.
pub fn extract_from_bytes_with_format(
    data: &[u8],
    format: ImageFormat,
    source_name: &str,
) -> Result<ImageMeta, MetaprobeError> {
    extract_from_bytes_with_format_options(data, format, source_name, true, None)
}

fn extract_from_bytes_with_format_options(
    data: &[u8],
    format: ImageFormat,
    source_name: &str,
    include_hash: bool,
    source_size: Option<u64>,
) -> Result<ImageMeta, MetaprobeError> {
    if format.is_video() {
        let mut meta = extract_video_from_metadata(
            data,
            format,
            source_size.unwrap_or(data.len() as u64),
            source_name,
        )?;
        if include_hash {
            meta.file_hash = Some(hash::sha256_hex(data));
        }
        return Ok(meta);
    }

    let dims = get_dimensions(data, format)
        .ok_or_else(|| MetaprobeError::DecodeFailed(source_name.to_string()))?;

    let icc_profile = extract_icc_profile(data, format);
    let color_space_from_icc = icc_profile.as_ref().map(|p| detect_color_space(p));
    let icc = icc_profile
        .as_ref()
        .map(|p| parse_icc_metadata(p))
        .unwrap_or_default();
    let exif = parse_exif(data, format);
    let color_space = color_space_from_icc
        .filter(|value| value != "Unknown")
        .unwrap_or_else(|| {
            exif.get("ColorSpace")
                .map(|value| value.trim().trim_matches('"').to_ascii_lowercase())
                .filter(|value| matches!(value.as_str(), "srgb" | "1"))
                .map(|_| "sRGB".to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        });

    Ok(MediaMeta {
        kind: "image".to_string(),
        format: format.as_str().to_string(),
        width: dims.width,
        height: dims.height,
        color_space,
        exif,
        icc,
        duration: None,
        creation_time: None,
        container_creation_time: None,
        codec: None,
        overall_bitrate: None,
        video_bitrate: None,
        frame_rate: None,
        metadata: HashMap::new(),
        file_hash: include_hash.then(|| hash::sha256_hex(data)),
    })
}

/// Extract video metadata from a metadata-only container slice.
pub fn extract_video_from_metadata(
    data: &[u8],
    format: ImageFormat,
    source_size: u64,
    source_name: &str,
) -> Result<ImageMeta, MetaprobeError> {
    if !format.is_video() {
        return Err(MetaprobeError::DecodeFailed(source_name.to_string()));
    }
    let video = parse_video_with_source_size(data, source_size, format)
        .ok_or_else(|| MetaprobeError::DecodeFailed(source_name.to_string()))?;
    Ok(MediaMeta {
        kind: "video".to_string(),
        format: format.as_str().to_string(),
        width: video.width,
        height: video.height,
        color_space: "Unknown".to_string(),
        exif: video.exif,
        icc: HashMap::new(),
        duration: video.duration,
        creation_time: video.creation_time,
        container_creation_time: video.container_creation_time,
        codec: video.codec,
        overall_bitrate: video.overall_bitrate,
        video_bitrate: video.video_bitrate,
        frame_rate: video.frame_rate,
        metadata: video.metadata,
        file_hash: None,
    })
}
