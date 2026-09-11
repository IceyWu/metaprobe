use metaprobe_core::{
    extract_from_bytes, extract_from_bytes_without_hash,
    extract_from_bytes_without_hash_with_source_size, MediaMeta,
};
use js_sys::{Object, Reflect};
use serde::Serialize;
use wasm_bindgen::prelude::*;

fn to_js<T: Serialize>(val: &T) -> Result<JsValue, JsValue> {
    let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
    val.serialize(&serializer)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

fn set_property(object: &Object, key: &str, value: JsValue) -> Result<(), JsValue> {
    Reflect::set(object, &JsValue::from_str(key), &value).map(|_| ())
}

fn set_string_map(
    object: &Object,
    key: &str,
    values: &std::collections::HashMap<String, String>,
) -> Result<(), JsValue> {
    let result = Object::new();
    for (entry_key, entry_value) in values {
        set_property(&result, entry_key, JsValue::from_str(entry_value))?;
    }
    set_property(object, key, result.into())
}

/// Build the hot-path result directly with JS primitives. The generic serde
/// serializer is convenient for batch/legacy APIs, but its recursive type
/// inspection is expensive for images with many EXIF tags.
fn media_meta_to_js(meta: &MediaMeta) -> Result<JsValue, JsValue> {
    let result = Object::new();
    set_property(&result, "kind", JsValue::from_str(&meta.kind))?;
    set_property(&result, "format", JsValue::from_str(&meta.format))?;
    set_property(&result, "width", JsValue::from_f64(meta.width as f64))?;
    set_property(&result, "height", JsValue::from_f64(meta.height as f64))?;
    set_property(&result, "colorSpace", JsValue::from_str(&meta.color_space))?;
    set_string_map(&result, "exif", &meta.exif)?;
    set_string_map(&result, "icc", &meta.icc)?;
    if let Some(value) = meta.duration {
        set_property(&result, "duration", JsValue::from_f64(value))?;
    }
    if let Some(value) = &meta.creation_time {
        set_property(&result, "creationTime", JsValue::from_str(value))?;
    }
    if let Some(value) = &meta.container_creation_time {
        set_property(&result, "containerCreationTime", JsValue::from_str(value))?;
    }
    if let Some(value) = &meta.codec {
        set_property(&result, "codec", JsValue::from_str(value))?;
    }
    if let Some(value) = meta.overall_bitrate {
        set_property(&result, "overallBitrate", JsValue::from_f64(value as f64))?;
    }
    if let Some(value) = meta.video_bitrate {
        set_property(&result, "videoBitrate", JsValue::from_f64(value as f64))?;
    }
    if let Some(value) = meta.frame_rate {
        set_property(&result, "frameRate", JsValue::from_f64(value))?;
    }
    set_string_map(&result, "metadata", &meta.metadata)?;
    if let Some(value) = &meta.file_hash {
        set_property(&result, "fileHash", JsValue::from_str(value))?;
    }
    Ok(result.into())
}

/// Extract metadata from image or video bytes in the browser.
///
/// Accepts a `Uint8Array` and a filename hint (for extension-based format fallback).
/// Returns a JS object with unified image/video metadata fields.
#[wasm_bindgen(js_name = "extractMeta")]
pub fn extract_meta(data: &[u8], filename: &str) -> Result<JsValue, JsValue> {
    let meta = extract_from_bytes(data, filename).map_err(|e| JsValue::from_str(&e.to_string()))?;

    to_js(&meta)
}

/// Extract metadata without hashing the input buffer.
///
/// This is intended for browser callers that can calculate SHA-256 with the
/// platform crypto API in parallel with metadata parsing.
#[wasm_bindgen(js_name = "extractMetaFast")]
pub fn extract_meta_fast(data: &[u8], filename: &str) -> Result<JsValue, JsValue> {
    let meta = extract_from_bytes_without_hash(data, filename)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    media_meta_to_js(&meta)
}

/// Extract metadata from a compact metadata slice while preserving the
/// original media size for video bitrate calculation.
#[wasm_bindgen(js_name = "extractMetaFastSized")]
pub fn extract_meta_fast_sized(
    data: &[u8],
    filename: &str,
    source_size: f64,
) -> Result<JsValue, JsValue> {
    let meta = extract_from_bytes_without_hash_with_source_size(data, filename, source_size as u64)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    media_meta_to_js(&meta)
}

/// Batch extract metadata from multiple images or videos.
///
/// Accepts an array of `{ data: Uint8Array, filename: string }` objects.
/// Returns an array of result objects with `success` discriminator.
#[wasm_bindgen(js_name = "extractMetaBatch")]
pub fn extract_meta_batch(items: JsValue) -> Result<JsValue, JsValue> {
    let entries: Vec<BatchInput> = serde_wasm_bindgen::from_value(items)
        .map_err(|e| JsValue::from_str(&format!("Invalid input: {}", e)))?;

    let results: Vec<BatchOutput> = entries
        .iter()
        .map(
            |entry| match extract_from_bytes(&entry.data, &entry.filename) {
                Ok(meta) => BatchOutput {
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
                Err(e) => BatchOutput {
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
                    path: Some(entry.filename.clone()),
                    message: Some(e.to_string()),
                },
            },
        )
        .collect();

    to_js(&results)
}

#[derive(serde::Deserialize)]
struct BatchInput {
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
    filename: String,
}

mod serde_bytes {
    use serde::Deserializer;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        // serde-wasm-bindgen passes Uint8Array as a sequence of numbers
        let v: Vec<u8> = serde::Deserialize::deserialize(d)?;
        Ok(v)
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchOutput {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color_space: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exif: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icc: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    creation_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    container_creation_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    overall_bitrate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    video_bitrate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frame_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}
