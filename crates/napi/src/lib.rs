use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use napi::bindgen_prelude::*;
use napi_derive::napi;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;

use metaprobe_core::error::MetaprobeError;
use metaprobe_core::format::{detect_format, format_from_extension, ImageFormat};
use metaprobe_core::{
    extract_from_bytes, extract_from_bytes_without_hash, extract_video_from_metadata,
    MediaMeta as CoreMediaMeta,
};

// ── napi types ──

#[napi(object)]
pub struct MediaMeta {
    pub kind: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub color_space: String,
    pub exif: HashMap<String, String>,
    pub icc: HashMap<String, String>,
    pub duration: Option<f64>,
    pub creation_time: Option<String>,
    pub container_creation_time: Option<String>,
    pub codec: Option<String>,
    pub overall_bitrate: Option<f64>,
    pub video_bitrate: Option<f64>,
    pub frame_rate: Option<f64>,
    pub metadata: HashMap<String, String>,
    pub file_hash: Option<String>,
}

#[napi(object)]
pub struct ExtractOptions {
    pub concurrency: Option<u32>,
    pub hash: Option<bool>,
}

#[napi(object)]
pub struct BatchResult {
    pub success: bool,
    pub kind: Option<String>,
    pub format: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub color_space: Option<String>,
    pub exif: Option<HashMap<String, String>>,
    pub icc: Option<HashMap<String, String>>,
    pub duration: Option<f64>,
    pub creation_time: Option<String>,
    pub container_creation_time: Option<String>,
    pub codec: Option<String>,
    pub overall_bitrate: Option<f64>,
    pub video_bitrate: Option<f64>,
    pub frame_rate: Option<f64>,
    pub metadata: Option<HashMap<String, String>>,
    pub file_hash: Option<String>,
    pub path: Option<String>,
    pub message: Option<String>,
}

// ── helpers ──

fn core_to_napi(m: CoreMediaMeta) -> MediaMeta {
    MediaMeta {
        kind: m.kind,
        format: m.format,
        width: m.width,
        height: m.height,
        color_space: m.color_space,
        exif: m.exif,
        icc: m.icc,
        duration: m.duration,
        creation_time: m.creation_time,
        container_creation_time: m.container_creation_time,
        codec: m.codec,
        overall_bitrate: m.overall_bitrate.map(|value| value as f64),
        video_bitrate: m.video_bitrate.map(|value| value as f64),
        frame_rate: m.frame_rate,
        metadata: m.metadata,
        file_hash: m.file_hash,
    }
}

fn extract_file(path: &str, include_hash: bool) -> Result<CoreMediaMeta> {
    let p = Path::new(path);

    if !p.exists() {
        return Err(napi::Error::from_reason(
            MetaprobeError::FileNotFound(path.to_string()).to_string(),
        ));
    }

    let data = fs::read(p).map_err(|e| napi::Error::from_reason(format!("IO error: {}", e)))?;

    let result = if include_hash {
        extract_from_bytes(&data, path)
    } else {
        extract_from_bytes_without_hash(&data, path)
    };
    result.map_err(|e| napi::Error::from_reason(e.to_string()))
}

fn detect_file_format(path: &str, header: &[u8]) -> Result<ImageFormat> {
    detect_format(header)
        .or_else(|| format_from_extension(path))
        .ok_or_else(|| {
            let ext = Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown");
            napi::Error::from_reason(MetaprobeError::UnsupportedFormat(ext.to_string()).to_string())
        })
}

fn extract_file_fast(path: &str) -> Result<CoreMediaMeta> {
    let mut file =
        fs::File::open(path).map_err(|e| napi::Error::from_reason(format!("IO error: {}", e)))?;
    let source_size = file
        .metadata()
        .map_err(|e| napi::Error::from_reason(format!("IO error: {}", e)))?
        .len();
    // 512 bytes covers signatures for MPEG-TS and keeps content detection
    // independent from the filename extension.
    let mut header = [0u8; 512];
    let mut header_reader = file
        .try_clone()
        .map_err(|e| napi::Error::from_reason(format!("IO error: {}", e)))?;
    let header_len = header_reader
        .read(&mut header)
        .map_err(|e| napi::Error::from_reason(format!("IO error: {}", e)))?;
    let format = detect_file_format(path, &header[..header_len])?;

    if format.is_video() {
        // ISO-BMFF files can be reduced to ftyp/moov for a fast metadata-only
        // parse. Other containers need their own header bytes and are handed
        // to the core fallback parser without an extension restriction.
        if matches!(format, ImageFormat::Mov | ImageFormat::Mp4) {
            if let Some(moov) = read_moov_box(file, source_size) {
                return extract_video_from_metadata(&moov, format, source_size, path)
                    .map_err(|e| napi::Error::from_reason(e.to_string()));
            }
        }
        let data = fs::read(path).map_err(|e| napi::Error::from_reason(format!("IO error: {}", e)))?;
        return extract_from_bytes_without_hash(&data, path)
            .map_err(|e| napi::Error::from_reason(e.to_string()));
    }

    if format == ImageFormat::Jpeg {
        let prefix = read_jpeg_metadata(&mut file, source_size)?;
        return extract_from_bytes_without_hash(&prefix, path)
            .map_err(|e| napi::Error::from_reason(e.to_string()));
    }

    let data = fs::read(path).map_err(|e| napi::Error::from_reason(format!("IO error: {}", e)))?;
    extract_from_bytes_without_hash(&data, path)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

fn jpeg_metadata_end(data: &[u8]) -> Option<usize> {
    if data.len() < 2 || data[..2] != [0xff, 0xd8] {
        return None;
    }
    let mut offset = 2usize;
    while offset + 3 < data.len() {
        while offset < data.len() && data[offset] != 0xff {
            offset += 1;
        }
        while offset < data.len() && data[offset] == 0xff {
            offset += 1;
        }
        let marker = *data.get(offset)?;
        let marker_start = offset.saturating_sub(1);
        offset += 1;
        if marker == 0xda {
            let length = u16::from_be_bytes([*data.get(offset)?, *data.get(offset + 1)?]) as usize;
            return Some((offset + length).min(data.len()));
        }
        if marker == 0xd9 {
            return Some(marker_start + 2);
        }
        if (0xd0..=0xd7).contains(&marker) || marker == 0x01 {
            continue;
        }
        let length = u16::from_be_bytes([*data.get(offset)?, *data.get(offset + 1)?]) as usize;
        if length < 2 || offset.checked_add(length)? > data.len() {
            return None;
        }
        offset += length;
    }
    None
}

fn read_jpeg_metadata(file: &mut fs::File, source_size: u64) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(0))
        .map_err(|error| napi::Error::from_reason(format!("IO error: {error}")))?;
    let mut data = Vec::new();
    let mut chunk = vec![0u8; 256 * 1024];
    while data.len() as u64 != source_size {
        let remaining = usize::try_from(source_size.saturating_sub(data.len() as u64))
            .unwrap_or(usize::MAX);
        let read_limit = chunk.len().min(remaining);
        let read = file
            .read(&mut chunk[..read_limit])
            .map_err(|error| napi::Error::from_reason(format!("IO error: {error}")))?;
        if read == 0 {
            break;
        }
        data.extend_from_slice(&chunk[..read]);
        if let Some(end) = jpeg_metadata_end(&data) {
            data.truncate(end);
            break;
        }
    }
    Ok(data)
}

fn read_moov_box(mut file: fs::File, source_size: u64) -> Option<Vec<u8>> {
    let mut offset = 0u64;
    while offset.checked_add(8)? <= source_size {
        file.seek(SeekFrom::Start(offset)).ok()?;
        let mut header = [0u8; 16];
        file.read_exact(&mut header[..8]).ok()?;
        let size32 = u32::from_be_bytes(header[..4].try_into().ok()?) as u64;
        let kind = [header[4], header[5], header[6], header[7]];
        let (box_size, header_size) = if size32 == 1 {
            file.read_exact(&mut header[8..16]).ok()?;
            (u64::from_be_bytes(header[8..16].try_into().ok()?), 16u64)
        } else if size32 == 0 {
            (source_size - offset, 8u64)
        } else {
            (size32, 8u64)
        };
        if box_size < header_size || box_size > source_size - offset {
            return None;
        }
        if kind == *b"moov" {
            let size = usize::try_from(box_size).ok()?;
            file.seek(SeekFrom::Start(offset)).ok()?;
            let mut moov = vec![0u8; size];
            file.read_exact(&mut moov).ok()?;
            return Some(moov);
        }
        offset = offset.checked_add(box_size)?;
    }
    None
}

// ── async tasks ──

pub struct ExtractMetaTask {
    path: String,
    include_hash: bool,
}

#[napi]
impl Task for ExtractMetaTask {
    type Output = CoreMediaMeta;
    type JsValue = MediaMeta;

    fn compute(&mut self) -> Result<Self::Output> {
        if self.include_hash {
            extract_file(&self.path, true)
        } else {
            extract_file_fast(&self.path)
        }
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(core_to_napi(output))
    }
}

pub enum BatchEntry {
    Success(Box<CoreMediaMeta>),
    Error { path: String, message: String },
}

pub struct ExtractMetaBatchTask {
    paths: Vec<String>,
    concurrency: Option<u32>,
    include_hash: bool,
}

#[napi]
impl Task for ExtractMetaBatchTask {
    type Output = Vec<BatchEntry>;
    type JsValue = Vec<BatchResult>;

    fn compute(&mut self) -> Result<Self::Output> {
        if self.paths.is_empty() {
            return Ok(Vec::new());
        }

        let run = || {
            self.paths
                .par_iter()
                .map(|path| {
                    let p = Path::new(path.as_str());
                    if !p.exists() {
                        return BatchEntry::Error {
                            path: path.clone(),
                            message: MetaprobeError::FileNotFound(path.clone()).to_string(),
                        };
                    }
                    let result = if self.include_hash {
                        extract_file(path, true)
                    } else {
                        extract_file_fast(path)
                    };
                    match result {
                            Ok(meta) => BatchEntry::Success(Box::new(meta)),
                            Err(e) => BatchEntry::Error {
                                path: path.clone(),
                                message: e.to_string(),
                            },
                    }
                })
                .collect()
        };

        if let Some(concurrency) = self.concurrency {
            let pool = ThreadPoolBuilder::new()
                .num_threads(concurrency.max(1) as usize)
                .build()
                .map_err(|error| napi::Error::from_reason(error.to_string()))?;
            Ok(pool.install(run))
        } else {
            Ok(run())
        }
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output
            .into_iter()
            .map(|entry| match entry {
                BatchEntry::Success(meta) => BatchResult {
                    success: true,
                    kind: Some(meta.kind),
                    format: Some(meta.format),
                    width: Some(meta.width),
                    height: Some(meta.height),
                    color_space: Some(meta.color_space),
                    exif: Some(meta.exif),
                    icc: Some(meta.icc),
                    duration: meta.duration,
                    creation_time: meta.creation_time,
                    container_creation_time: meta.container_creation_time,
                    codec: meta.codec,
                    overall_bitrate: meta.overall_bitrate.map(|value| value as f64),
                    video_bitrate: meta.video_bitrate.map(|value| value as f64),
                    frame_rate: meta.frame_rate,
                    metadata: Some(meta.metadata),
                    file_hash: meta.file_hash,
                    path: None,
                    message: None,
                },
                BatchEntry::Error { path, message } => BatchResult {
                    success: false,
                    kind: None,
                    format: None,
                    width: None,
                    height: None,
                    color_space: None,
                    exif: None,
                    icc: None,
                    duration: None,
                    creation_time: None,
                    container_creation_time: None,
                    codec: None,
                    overall_bitrate: None,
                    video_bitrate: None,
                    frame_rate: None,
                    metadata: None,
                    file_hash: None,
                    path: Some(path),
                    message: Some(message),
                },
            })
            .collect())
    }
}

// ── exported functions ──

#[napi]
pub fn extract_meta(path: String, options: Option<ExtractOptions>) -> AsyncTask<ExtractMetaTask> {
    AsyncTask::new(ExtractMetaTask {
        path,
        include_hash: options.and_then(|value| value.hash).unwrap_or(true),
    })
}

#[napi]
pub fn extract_meta_batch(
    paths: Vec<String>,
    options: Option<ExtractOptions>,
) -> AsyncTask<ExtractMetaBatchTask> {
    let concurrency = options.as_ref().and_then(|value| value.concurrency);
    let include_hash = options.and_then(|value| value.hash).unwrap_or(true);
    AsyncTask::new(ExtractMetaBatchTask {
        paths,
        concurrency,
        include_hash,
    })
}
