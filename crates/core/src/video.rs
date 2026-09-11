use std::collections::HashMap;
use std::convert::TryInto;

use crate::exif_parser::{normalize_exif_map, parse_exif};
use crate::format::ImageFormat;

#[derive(Debug, Clone)]
pub struct VideoMeta {
    pub width: u32,
    pub height: u32,
    pub duration: Option<f64>,
    pub creation_time: Option<String>,
    pub container_creation_time: Option<String>,
    pub codec: Option<String>,
    pub overall_bitrate: Option<u64>,
    pub video_bitrate: Option<u64>,
    pub frame_rate: Option<f64>,
    pub exif: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Default)]
struct TrackMeta {
    width: u32,
    height: u32,
    timescale: u64,
    duration_units: u64,
    sample_count: u64,
    sample_bytes: u64,
    codec: Option<&'static str>,
}

/// Parse MOV/MP4 metadata directly from the ISO-BMFF container.
/// No video frames are decoded and no external binary is required.
pub fn parse_video(data: &[u8], _format: ImageFormat) -> Option<VideoMeta> {
    parse_video_with_source_size(data, data.len() as u64, _format)
}

/// Parse a metadata-only container slice while retaining the original file
/// size for bitrate calculation.
pub fn parse_video_with_source_size(
    data: &[u8],
    source_size: u64,
    format: ImageFormat,
) -> Option<VideoMeta> {
    let Some(moov) = find_box(data, b"moov") else {
        return parse_generic_video(data, source_size, format);
    };
    let (movie_timescale, movie_duration, movie_creation) =
        parse_mvhd(find_direct_box(moov, b"mvhd"));

    let mut track = TrackMeta::default();
    for_each_box(moov, |kind, payload| {
        if kind == b"trak" {
            if let Some(candidate) = parse_video_track(payload) {
                if candidate.width.saturating_mul(candidate.height)
                    >= track.width.saturating_mul(track.height)
                {
                    track = candidate;
                }
            }
        }
    });

    let mut metadata = HashMap::new();
    parse_metadata_tree(moov, &mut metadata);
    if let Some(value) = movie_creation.clone() {
        metadata.insert("container_creation_time".to_string(), value);
    }

    let mut exif = find_embedded_exif(moov);
    if let Some(mvtg) = find_box(moov, b"MVTG") {
        for (key, value) in parse_fuji_mvtg(mvtg) {
            exif.entry(key).or_insert(value);
        }
        normalize_exif_map(&mut exif);
    }
    let recorded_time = first_metadata(&metadata, &["creation_date", "©day"])
        .and_then(|value| normalize_quicktime_datetime(&value))
        .or_else(|| exif.get("DateTimeOriginalISO").cloned())
        .or_else(|| {
            exif.get("DateTimeOriginal")
                .and_then(|value| normalize_exif_datetime(value, exif.get("OffsetTimeOriginal")))
        });
    if let Some(value) = recorded_time.clone() {
        metadata.insert("recorded_at".to_string(), value);
    }
    let creation_time = recorded_time.clone().or(movie_creation.clone());
    let duration = if track.timescale > 0 && track.duration_units > 0 {
        Some(track.duration_units as f64 / track.timescale as f64)
    } else if movie_timescale > 0 && movie_duration > 0 {
        Some(movie_duration as f64 / movie_timescale as f64)
    } else {
        None
    };
    let frame_rate = if track.sample_count > 0 {
        duration
            .filter(|value| *value > 0.0)
            .map(|value| track.sample_count as f64 / value)
    } else {
        None
    };
    let overall_bitrate = duration
        .filter(|value| *value > 0.0)
        .map(|value| ((source_size as f64 * 8.0) / value).round() as u64);
    let video_bitrate = duration
        .filter(|value| *value > 0.0 && track.sample_bytes > 0)
        .map(|value| ((track.sample_bytes as f64 * 8.0) / value).round() as u64);

    let location = first_metadata(
        &metadata,
        &["location", "©xyz", "com.apple.quicktime.location.ISO6709"],
    )
    .and_then(|value| parse_location(&value))
    // Location text lives in the QuickTime metadata tree. Searching the
    // complete movie here would allocate and scan every encoded frame;
    // keep the fallback bounded to the already extracted `moov` box.
    .or_else(|| find_embedded_location(moov));
    if let Some((latitude, longitude, altitude)) = location {
        exif.entry("GPSLatitude".to_string())
            .or_insert_with(|| latitude.to_string());
        exif.entry("GPSLongitude".to_string())
            .or_insert_with(|| longitude.to_string());
        exif.entry("latitude".to_string())
            .or_insert_with(|| latitude.to_string());
        exif.entry("longitude".to_string())
            .or_insert_with(|| longitude.to_string());
        if let Some(altitude) = altitude {
            exif.entry("GPSAltitude".to_string())
                .or_insert_with(|| altitude.to_string());
            exif.entry("altitude".to_string())
                .or_insert_with(|| altitude.to_string());
        }
    }
    if let Some(make) = first_metadata(&metadata, &["make", "©mak"]) {
        exif.entry("Make".to_string()).or_insert(make);
    }
    if let Some(model) = first_metadata(&metadata, &["model", "©mod"]) {
        exif.entry("Model".to_string()).or_insert(model);
    }
    if let Some(software) = first_metadata(&metadata, &["software", "©swr"]) {
        exif.entry("Software".to_string()).or_insert(software);
    }
    if let Some(lens_model) = first_metadata(&metadata, &["lens_model"]) {
        exif.entry("LensModel".to_string()).or_insert(lens_model);
    }
    if let Some(date) = recorded_time {
        exif.entry("DateTimeOriginal".to_string())
            .or_insert_with(|| date.clone());
        exif.entry("DateTimeOriginalISO".to_string())
            .or_insert(date);
    }

    Some(VideoMeta {
        width: track.width,
        height: track.height,
        duration,
        creation_time,
        container_creation_time: movie_creation,
        codec: track.codec.map(str::to_string),
        overall_bitrate,
        video_bitrate,
        frame_rate,
        exif,
        metadata,
    })
}

/// Return a useful container-level result for video formats that do not use
/// ISO-BMFF. The parser intentionally does not decode frames; when a compact
/// container header exposes dimensions/timing we report them, otherwise the
/// values remain zero/unknown rather than rejecting the file because of its
/// extension.
fn parse_generic_video(data: &[u8], source_size: u64, format: ImageFormat) -> Option<VideoMeta> {
    let mut width = 0;
    let mut height = 0;
    let mut duration = None;
    let mut frame_rate = None;
    let mut codec = None;
    let mut metadata = HashMap::new();
    metadata.insert("container".to_string(), format.as_str().to_string());

    if format == ImageFormat::Avi {
        if let Some(avih) = find_riff_chunk(data, b"avih") {
            if avih.len() >= 40 {
                let microseconds_per_frame = read_le_u32(avih, 0).unwrap_or(0);
                let total_frames = read_le_u32(avih, 16).unwrap_or(0);
                width = read_le_u32(avih, 32).unwrap_or(0);
                height = read_le_u32(avih, 36).unwrap_or(0);
                if microseconds_per_frame > 0 {
                    duration = Some(
                        total_frames as f64 * microseconds_per_frame as f64 / 1_000_000.0,
                    );
                    frame_rate = Some(1_000_000.0 / microseconds_per_frame as f64);
                }
                metadata.insert("frame_count".to_string(), total_frames.to_string());
            }
        }
        codec = find_riff_chunk(data, b"strh")
            .and_then(|stream| stream.get(4..8))
            .and_then(|value| std::str::from_utf8(value).ok())
            .map(str::to_string);
    }

    if width > 0 && height > 0 {
        metadata.insert("width".to_string(), width.to_string());
        metadata.insert("height".to_string(), height.to_string());
    }
    let overall_bitrate = duration
        .filter(|value| *value > 0.0)
        .map(|value| ((source_size as f64 * 8.0) / value).round() as u64);

    Some(VideoMeta {
        width,
        height,
        duration,
        creation_time: None,
        container_creation_time: None,
        codec,
        overall_bitrate,
        video_bitrate: None,
        frame_rate,
        exif: HashMap::new(),
        metadata,
    })
}

fn find_riff_chunk<'a>(data: &'a [u8], target: &[u8; 4]) -> Option<&'a [u8]> {
    if data.len() < 12 || &data[..4] != b"RIFF" {
        return None;
    }
    let mut offset = 12;
    while let Some(relative) = data[offset..].windows(4).position(|window| window == target) {
        let chunk_start = offset + relative;
        let payload_start = chunk_start + 8;
        let size = read_le_u32(data, chunk_start + 4)? as usize;
        let payload_end = payload_start.checked_add(size)?.min(data.len());
        if payload_start <= data.len() {
            return Some(&data[payload_start..payload_end]);
        }
        offset = chunk_start + 4;
    }
    None
}

fn read_le_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?))
}

fn parse_video_track(track: &[u8]) -> Option<TrackMeta> {
    let handler = find_box(track, b"hdlr")?;
    if handler.len() < 12 || &handler[8..12] != b"vide" {
        return None;
    }
    let (width, height) = find_box(track, b"tkhd")
        .and_then(parse_tkhd_dimensions)
        .unwrap_or((0, 0));
    let (timescale, duration_units) = find_box(track, b"mdhd")
        .and_then(parse_mdhd)
        .unwrap_or((0, 0));
    let codec = find_box(track, b"stsd").and_then(parse_codec);
    let sample_count = find_box(track, b"stts").map(parse_stts).unwrap_or(0);
    let sample_bytes = find_box(track, b"stsz").map(parse_stsz).unwrap_or(0);
    Some(TrackMeta {
        width,
        height,
        timescale,
        duration_units,
        sample_count,
        sample_bytes,
        codec,
    })
}

fn parse_mvhd(data: Option<&[u8]>) -> (u64, u64, Option<String>) {
    let Some(data) = data else {
        return (0, 0, None);
    };
    let version = *data.first().unwrap_or(&0);
    let creation = if version == 1 {
        read_u64(data, 4).and_then(quicktime_seconds_to_rfc3339)
    } else {
        read_u32(data, 4)
            .map(u64::from)
            .and_then(quicktime_seconds_to_rfc3339)
    };
    let timescale_offset = if version == 1 { 20 } else { 12 };
    let duration_offset = if version == 1 { 24 } else { 16 };
    let Some(timescale) = read_u32(data, timescale_offset) else {
        return (0, 0, None);
    };
    let duration = if version == 1 {
        read_u64(data, duration_offset).unwrap_or(0)
    } else {
        read_u32(data, duration_offset).unwrap_or(0) as u64
    };
    (timescale as u64, duration, creation)
}

fn quicktime_seconds_to_rfc3339(seconds: u64) -> Option<String> {
    const QUICKTIME_UNIX_OFFSET: u64 = 2_082_844_800;
    if seconds < QUICKTIME_UNIX_OFFSET {
        return None;
    }
    let unix = seconds - QUICKTIME_UNIX_OFFSET;
    let days = (unix / 86_400) as i64;
    let seconds_of_day = unix % 86_400;

    // Gregorian civil date conversion, using 1970-01-01 as day zero.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let month_part = (5 * doy + 2) / 153;
    let day = doy - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };

    Some(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds_of_day / 3_600,
        (seconds_of_day % 3_600) / 60,
        seconds_of_day % 60
    ))
}

fn parse_tkhd_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 8 {
        return None;
    }
    let width = read_u32(data, data.len() - 8)? >> 16;
    let height = read_u32(data, data.len() - 4)? >> 16;
    Some((width, height))
}

fn parse_mdhd(data: &[u8]) -> Option<(u64, u64)> {
    let version = *data.first()?;
    let timescale_offset = if version == 1 { 20 } else { 12 };
    let duration_offset = if version == 1 { 24 } else { 16 };
    let timescale = read_u32(data, timescale_offset)? as u64;
    let duration = if version == 1 {
        read_u64(data, duration_offset)?
    } else {
        read_u32(data, duration_offset)? as u64
    };
    Some((timescale, duration))
}

fn parse_codec(data: &[u8]) -> Option<&'static str> {
    if data.len() < 16 || read_u32(data, 4)? == 0 {
        return None;
    }
    Some(match &data[12..16] {
        b"avc1" | b"avc3" => "AVC",
        b"hvc1" | b"hev1" => "HEVC",
        b"av01" => "AV1",
        b"vp09" => "VP9",
        b"mp4v" => "MPEG-4 Visual",
        b"jpeg" => "JPEG",
        _ => "Unknown",
    })
}

fn parse_stts(data: &[u8]) -> u64 {
    let Some(entry_count) = read_u32(data, 4) else {
        return 0;
    };
    let mut offset = 8usize;
    let mut count = 0u64;
    for _ in 0..entry_count {
        let Some(sample_count) = read_u32(data, offset) else {
            break;
        };
        count = count.saturating_add(sample_count as u64);
        offset = offset.saturating_add(8);
    }
    count
}

fn parse_stsz(data: &[u8]) -> u64 {
    let Some(sample_size) = read_u32(data, 4) else {
        return 0;
    };
    let Some(sample_count) = read_u32(data, 8) else {
        return 0;
    };
    if sample_size > 0 {
        return u64::from(sample_size).saturating_mul(u64::from(sample_count));
    }

    let mut total = 0u64;
    let mut offset = 12usize;
    for _ in 0..sample_count {
        let Some(size) = read_u32(data, offset) else {
            break;
        };
        total = total.saturating_add(u64::from(size));
        offset = offset.saturating_add(4);
    }
    total
}

fn parse_metadata_tree(data: &[u8], metadata: &mut HashMap<String, String>) {
    for (kind, payload) in BoxIter::new(data) {
        if kind == b"meta" {
            let meta = container_payload(kind, payload);
            let keys = find_direct_box(meta, b"keys")
                .map(parse_keys)
                .unwrap_or_default();
            if let Some(ilst) = find_direct_box(meta, b"ilst") {
                parse_ilst(ilst, &keys, metadata);
            }
            parse_metadata_tree(meta, metadata);
        } else if is_container(kind) && kind != b"ilst" {
            parse_metadata_tree(container_payload(kind, payload), metadata);
        }
    }
}

fn parse_keys(data: &[u8]) -> Vec<String> {
    let Some(entry_count) = read_u32(data, 4) else {
        return Vec::new();
    };
    let mut keys = Vec::with_capacity(entry_count as usize);
    let mut offset = 8usize;
    for _ in 0..entry_count {
        let Some(size) = read_u32(data, offset).map(|value| value as usize) else {
            break;
        };
        if size < 8 || offset.saturating_add(size) > data.len() {
            break;
        }
        keys.push(decode_text(&data[offset + 8..offset + size]));
        offset += size;
    }
    keys
}

fn parse_ilst(data: &[u8], keys: &[String], metadata: &mut HashMap<String, String>) {
    for_each_box(data, |kind, item| {
        let raw_key = fourcc_to_string(kind);
        let index = read_u32(kind, 0).unwrap_or(0) as usize;
        let key = if index > 0 && index <= keys.len() {
            keys[index - 1].clone()
        } else if kind == b"----" {
            let name = find_direct_box(item, b"name")
                .map(decode_text)
                .unwrap_or_default();
            let mean = find_direct_box(item, b"mean")
                .map(decode_text)
                .unwrap_or_default();
            if mean.is_empty() {
                name
            } else if name.is_empty() {
                mean
            } else {
                format!("{mean}:{name}")
            }
        } else {
            raw_key.clone()
        };
        let Some(value_box) = find_direct_box(item, b"data") else {
            return;
        };
        if value_box.len() < 8 {
            return;
        }
        let value = decode_data_value(value_box);
        if value.is_empty() {
            return;
        }
        if raw_key.chars().all(|character| !character.is_control()) {
            metadata.insert(raw_key, value.clone());
        }
        if !key.is_empty() {
            metadata.insert(key.clone(), value.clone());
        }
        let normalized_key = key.to_ascii_lowercase();
        if normalized_key.contains("location.iso6709") || normalized_key == "©xyz" {
            metadata.insert("location".to_string(), value.clone());
        } else if normalized_key.ends_with("make") {
            metadata.insert("make".to_string(), value.clone());
        } else if normalized_key.ends_with("model") {
            metadata.insert("model".to_string(), value.clone());
        } else if normalized_key.ends_with("software") {
            metadata.insert("software".to_string(), value.clone());
        } else if normalized_key.ends_with("camera.lens_model") {
            metadata.insert("lens_model".to_string(), value.clone());
        } else if normalized_key.ends_with("creationdate") || normalized_key == "©day" {
            metadata.insert("creation_date".to_string(), value);
        }
    });
}

fn decode_data_value(data: &[u8]) -> String {
    if data.len() < 8 {
        return String::new();
    }
    let data_type = read_u32(data, 0).unwrap_or(0) & 0x00ff_ffff;
    let value = &data[8..];
    match data_type {
        1 | 0 => decode_text(value),
        2 => {
            let chars: Vec<u16> = value
                .chunks_exact(2)
                .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                .collect();
            String::from_utf16_lossy(&chars)
                .trim_matches('\0')
                .trim()
                .to_string()
        }
        21 if !value.is_empty() && value.len() <= 8 => read_signed_integer(value).to_string(),
        22 if !value.is_empty() && value.len() <= 8 => read_unsigned_integer(value).to_string(),
        23 if value.len() == 4 => read_u32(value, 0)
            .map(f32::from_bits)
            .map(|number| number.to_string())
            .unwrap_or_default(),
        24 if value.len() == 8 => read_u64(value, 0)
            .map(f64::from_bits)
            .map(|number| number.to_string())
            .unwrap_or_default(),
        _ => decode_text(value),
    }
}

fn read_signed_integer(data: &[u8]) -> i64 {
    let mut bytes = if data.first().is_some_and(|value| value & 0x80 != 0) {
        [0xff; 8]
    } else {
        [0; 8]
    };
    bytes[8 - data.len()..].copy_from_slice(data);
    i64::from_be_bytes(bytes)
}

fn read_unsigned_integer(data: &[u8]) -> u64 {
    let mut bytes = [0; 8];
    bytes[8 - data.len()..].copy_from_slice(data);
    u64::from_be_bytes(bytes)
}

fn first_metadata(metadata: &HashMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| metadata.get(*key).cloned())
}

fn normalize_quicktime_datetime(value: &str) -> Option<String> {
    let value = value.trim().trim_matches('\0').trim_matches('"');
    if value.len() < 19 {
        return None;
    }
    let mut normalized = value.to_string();
    let bytes = normalized.as_bytes();
    if normalized.len() >= 24
        && matches!(bytes[normalized.len() - 5], b'+' | b'-')
        && bytes[normalized.len() - 4..].iter().all(u8::is_ascii_digit)
    {
        normalized.insert(normalized.len() - 2, ':');
    }
    Some(normalized)
}

fn normalize_exif_datetime(value: &str, offset: Option<&String>) -> Option<String> {
    let value = value.trim().trim_matches('"');
    let (date, time) = value.split_once(' ')?;
    let date = date.replace(':', "-");
    if date.len() != 10 || time.len() < 8 {
        return None;
    }
    let timezone = offset
        .map(|value| value.trim().trim_matches('"'))
        .filter(|value| value.len() == 6 && matches!(value.as_bytes()[0], b'+' | b'-'))
        .unwrap_or("Z");
    Some(format!("{date}T{}{timezone}", &time[..8]))
}

fn parse_fuji_mvtg(data: &[u8]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if data.len() < 18 {
        return map;
    }
    // FUJIFILM's MVTG atom carries a headerless little-endian TIFF directory;
    // offsets are relative to the first IFD count after the 16-byte header.
    let base = &data[16..];
    parse_fuji_ifd(base, 0, false, 0, &mut map);
    map
}

fn parse_fuji_ifd(
    base: &[u8],
    offset: usize,
    gps: bool,
    depth: usize,
    map: &mut HashMap<String, String>,
) {
    if depth > 4 || offset + 2 > base.len() {
        return;
    }
    let count = read_u16_le(base, offset).unwrap_or(0) as usize;
    for index in 0..count.min(512) {
        let entry_offset = offset + 2 + index * 12;
        let Some(entry) = base.get(entry_offset..entry_offset + 12) else {
            break;
        };
        let tag = read_u16_le(entry, 0).unwrap_or(0);
        let field_type = read_u16_le(entry, 2).unwrap_or(0);
        let item_count = read_u32_le(entry, 4).unwrap_or(0) as usize;
        let value_offset = read_u32_le(entry, 8).unwrap_or(0) as usize;
        if tag == 0x8769 || tag == 0x8825 {
            parse_fuji_ifd(base, value_offset, tag == 0x8825, depth + 1, map);
            continue;
        }
        let Some(name) = fuji_tag_name(tag, gps) else {
            continue;
        };
        let Some(value) = parse_fuji_value(base, entry, field_type, item_count) else {
            continue;
        };
        if !value.is_empty() {
            map.insert(name.to_string(), value);
        }
    }
}

fn fuji_tag_name(tag: u16, gps: bool) -> Option<&'static str> {
    if gps {
        return match tag {
            0x0001 => Some("GPSLatitudeRef"),
            0x0002 => Some("GPSLatitude"),
            0x0003 => Some("GPSLongitudeRef"),
            0x0004 => Some("GPSLongitude"),
            0x0005 => Some("GPSAltitudeRef"),
            0x0006 => Some("GPSAltitude"),
            0x0007 => Some("GPSTimeStamp"),
            0x001d => Some("GPSDateStamp"),
            _ => None,
        };
    }
    match tag {
        0x010f => Some("Make"),
        0x0110 => Some("Model"),
        0x0112 => Some("Orientation"),
        0x0131 => Some("Software"),
        0x0132 => Some("DateTime"),
        0x829a => Some("ExposureTime"),
        0x829d => Some("FNumber"),
        0x8827 => Some("PhotographicSensitivity"),
        0x8830 => Some("SensitivityType"),
        0x9003 => Some("DateTimeOriginal"),
        0x9004 => Some("DateTimeDigitized"),
        0x9010 => Some("OffsetTime"),
        0x9011 => Some("OffsetTimeOriginal"),
        0x9012 => Some("OffsetTimeDigitized"),
        0x9201 => Some("ShutterSpeedValue"),
        0x9202 => Some("ApertureValue"),
        0x9203 => Some("BrightnessValue"),
        0x9204 => Some("ExposureBiasValue"),
        0x9205 => Some("MaxApertureValue"),
        0x9207 => Some("MeteringMode"),
        0x9208 => Some("LightSource"),
        0x9209 => Some("Flash"),
        0x920a => Some("FocalLength"),
        0xa001 => Some("ColorSpace"),
        0xa002 => Some("PixelXDimension"),
        0xa003 => Some("PixelYDimension"),
        0xa405 => Some("FocalLengthIn35mmFilm"),
        0xa431 => Some("BodySerialNumber"),
        0xa434 => Some("LensModel"),
        _ => None,
    }
}

fn parse_fuji_value(
    base: &[u8],
    entry: &[u8],
    field_type: u16,
    count: usize,
) -> Option<String> {
    let unit = match field_type {
        1 | 2 | 7 => 1usize,
        3 => 2,
        4 | 9 => 4,
        5 | 10 => 8,
        _ => return None,
    };
    let size = unit.checked_mul(count)?;
    let value = if size <= 4 {
        entry.get(8..8 + size)?
    } else {
        let offset = read_u32_le(entry, 8)? as usize;
        base.get(offset..offset.checked_add(size)?)?
    };
    match field_type {
        2 => Some(decode_text(value)),
        1 | 7 => {
            let text = decode_text(value);
            if text.is_empty() {
                Some(value.iter().map(u8::to_string).collect::<Vec<_>>().join(", "))
            } else {
                Some(text)
            }
        }
        3 => Some(
            value
                .chunks_exact(2)
                .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]).to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        4 => Some(
            value
                .chunks_exact(4)
                .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap_or_default()).to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        5 => Some(
            value
                .chunks_exact(8)
                .map(|bytes| {
                    let numerator = u32::from_le_bytes(bytes[..4].try_into().unwrap_or_default());
                    let denominator = u32::from_le_bytes(bytes[4..].try_into().unwrap_or_default());
                    if denominator == 0 { 0.0 } else { numerator as f64 / denominator as f64 }
                })
                .map(|number| number.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        9 => Some(
            value
                .chunks_exact(4)
                .map(|bytes| i32::from_le_bytes(bytes.try_into().unwrap_or_default()).to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        10 => Some(
            value
                .chunks_exact(8)
                .map(|bytes| {
                    let numerator = i32::from_le_bytes(bytes[..4].try_into().unwrap_or_default());
                    let denominator = i32::from_le_bytes(bytes[4..].try_into().unwrap_or_default());
                    if denominator == 0 { 0.0 } else { numerator as f64 / denominator as f64 }
                })
                .map(|number| number.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        _ => None,
    }
}

fn read_u16_le(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(offset..offset + 2)?.try_into().ok()?))
}

fn read_u32_le(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?))
}

/// Some cameras put a complete TIFF/EXIF payload inside a QuickTime metadata
/// atom. Locate those signatures and keep the richest successfully parsed set.
fn find_embedded_exif(data: &[u8]) -> HashMap<String, String> {
    let mut best = HashMap::new();
    let mut candidates = 0usize;
    for index in 0..data.len().saturating_sub(4) {
        if !matches!(&data[index..index + 4], b"II*\0" | b"MM\0*") {
            continue;
        }
        let parsed = parse_exif(&data[index..], ImageFormat::Jpeg);
        if parsed.len() > best.len() {
            best = parsed;
        }
        candidates += 1;
        if candidates >= 64 {
            break;
        }
    }
    best
}

fn find_embedded_location(data: &[u8]) -> Option<(f64, f64, Option<f64>)> {
    let normalized: String = String::from_utf8_lossy(data)
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    for index in 0..tokens.len().saturating_sub(1) {
        if !is_coordinate_token(tokens[index]) || !is_coordinate_token(tokens[index + 1]) {
            continue;
        }
        let end = if tokens
            .get(index + 2)
            .is_some_and(|token| token.ends_with('m'))
        {
            index + 2
        } else {
            index + 1
        };
        if let Some(parsed) = parse_location(&tokens[index..=end].join(" ")) {
            return Some(parsed);
        }
    }
    None
}

fn is_coordinate_token(token: &str) -> bool {
    token.contains('°')
        && token.chars().last().is_some_and(|character| {
            matches!(character.to_ascii_uppercase(), 'N' | 'S' | 'E' | 'W')
        })
}

fn parse_location(value: &str) -> Option<(f64, f64, Option<f64>)> {
    let value = value.trim();
    if value.contains('°') {
        let parts: Vec<&str> = value.split_whitespace().collect();
        if parts.len() >= 2 {
            let latitude = parse_coordinate_token(parts[0])?;
            let longitude = parse_coordinate_token(parts[1])?;
            let altitude = parts
                .get(2)
                .and_then(|part| part.trim_end_matches('m').parse::<f64>().ok());
            return Some((latitude, longitude, altitude));
        }
    }

    let mut values = Vec::new();
    let mut start = None;
    for (index, ch) in value.char_indices() {
        if (ch == '+' || ch == '-') && start.is_some() {
            let begin = start.take()?;
            values.push(
                value[begin..index]
                    .trim_end_matches('/')
                    .parse::<f64>()
                    .ok()?,
            );
            start = Some(index);
        } else if (ch == '+' || ch == '-') && start.is_none() {
            start = Some(index);
        }
    }
    if let Some(begin) = start {
        values.push(value[begin..].trim_end_matches('/').parse::<f64>().ok()?);
    }
    if values.len() >= 2 {
        Some((values[0], values[1], values.get(2).copied()))
    } else {
        None
    }
}

fn parse_coordinate_token(token: &str) -> Option<f64> {
    let token = token.trim();
    let hemisphere = token.chars().last()?;
    if !matches!(hemisphere.to_ascii_uppercase(), 'N' | 'S' | 'E' | 'W') {
        return None;
    }
    coordinate_with_hemisphere(
        token[..token.len() - hemisphere.len_utf8()].trim_end_matches('°'),
        &hemisphere.to_string(),
    )
}

fn coordinate_with_hemisphere(number: &str, hemisphere: &str) -> Option<f64> {
    let value = number.trim_end_matches('°').parse::<f64>().ok()?;
    Some(
        if hemisphere.eq_ignore_ascii_case("S") || hemisphere.eq_ignore_ascii_case("W") {
            -value
        } else {
            value
        },
    )
}

fn decode_text(data: &[u8]) -> String {
    String::from_utf8_lossy(data)
        .trim_matches('\0')
        .trim()
        .to_string()
}

fn fourcc_to_string(kind: &[u8]) -> String {
    String::from_utf8_lossy(kind).trim_matches('\0').to_string()
}

fn is_container(kind: &[u8]) -> bool {
    matches!(
        kind,
        b"moov"
            | b"trak"
            | b"mdia"
            | b"minf"
            | b"stbl"
            | b"udta"
            | b"meta"
            | b"ilst"
            | b"edts"
            | b"dinf"
    )
}

fn container_payload<'a>(kind: &[u8], payload: &'a [u8]) -> &'a [u8] {
    if kind != b"meta" {
        return payload;
    }
    // ISO FullBox `meta` starts with version/flags, while QuickTime's older
    // variant starts directly with child atoms. Accept both forms.
    let starts_with_child = read_u32(payload, 0)
        .is_some_and(|size| size >= 8 && size as usize <= payload.len())
        && payload
            .get(4..8)
            .is_some_and(|name| name.iter().all(|byte| byte.is_ascii_graphic()));
    if starts_with_child {
        payload
    } else {
        payload.get(4..).unwrap_or_default()
    }
}

fn find_box<'a>(data: &'a [u8], target: &[u8; 4]) -> Option<&'a [u8]> {
    for (kind, payload) in BoxIter::new(data) {
        if kind == target {
            return Some(payload);
        }
    }
    for (kind, payload) in BoxIter::new(data) {
        if is_container(kind) {
            if let Some(found) = find_box(container_payload(kind, payload), target) {
                return Some(found);
            }
        }
    }
    None
}

fn find_direct_box<'a>(data: &'a [u8], target: &[u8; 4]) -> Option<&'a [u8]> {
    for (kind, payload) in BoxIter::new(data) {
        if kind == target {
            return Some(payload);
        }
    }
    None
}

fn for_each_box(data: &[u8], mut callback: impl FnMut(&[u8], &[u8])) {
    for (kind, payload) in BoxIter::new(data) {
        callback(kind, payload);
    }
}

struct BoxIter<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> BoxIter<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }
}

impl<'a> Iterator for BoxIter<'a> {
    type Item = (&'a [u8], &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        let offset = self.offset;
        if offset.saturating_add(8) > self.data.len() {
            return None;
        }
        let size32 = read_u32(self.data, offset)? as u64;
        let header_size = if size32 == 1 { 16 } else { 8 };
        if offset.saturating_add(header_size) > self.data.len() {
            return None;
        }
        let size = if size32 == 1 {
            read_u64(self.data, offset + 8).unwrap_or(0)
        } else if size32 == 0 {
            (self.data.len() - offset) as u64
        } else {
            size32
        };
        if size < header_size as u64 || size > (self.data.len() - offset) as u64 {
            return None;
        }
        let end = offset + size as usize;
        self.offset = end;
        Some((
            &self.data[offset + 4..offset + 8],
            &self.data[offset + header_size..end],
        ))
    }
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..offset + 4)?;
    Some(u32::from_be_bytes(bytes.try_into().ok()?))
}

fn read_u64(data: &[u8], offset: usize) -> Option<u64> {
    let bytes = data.get(offset..offset + 8)?;
    Some(u64::from_be_bytes(bytes.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::{
        find_embedded_location, normalize_exif_datetime, normalize_quicktime_datetime,
        parse_location, parse_metadata_tree, quicktime_seconds_to_rfc3339,
    };
    use std::collections::HashMap;

    fn atom(kind: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut result = Vec::with_capacity(payload.len() + 8);
        result.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
        result.extend_from_slice(&kind);
        result.extend_from_slice(payload);
        result
    }

    #[test]
    fn parses_recorded_location() {
        let parsed = parse_location("29.5789°N 103.4644°E 461.150m").unwrap();
        assert!((parsed.0 - 29.5789).abs() < f64::EPSILON);
        assert!((parsed.1 - 103.4644).abs() < f64::EPSILON);
        assert_eq!(parsed.2, Some(461.15));
    }

    #[test]
    fn parses_iso6709_location() {
        let parsed = parse_location("+29.5789+103.4644+461.150/").unwrap();
        assert!((parsed.0 - 29.5789).abs() < f64::EPSILON);
        assert!((parsed.1 - 103.4644).abs() < f64::EPSILON);
        assert_eq!(parsed.2, Some(461.15));
    }

    #[test]
    fn applies_south_and_west_signs() {
        let parsed = parse_location("10.0°S 20.0°W").unwrap();
        assert_eq!(parsed.0, -10.0);
        assert_eq!(parsed.1, -20.0);
    }

    #[test]
    fn finds_location_embedded_in_quicktime_text() {
        let parsed = find_embedded_location(
            b"random\0metadata 29.5789\xC2\xB0N 103.4644\xC2\xB0E 461.150m\0tail",
        )
        .unwrap();
        assert!((parsed.0 - 29.5789).abs() < f64::EPSILON);
        assert!((parsed.1 - 103.4644).abs() < f64::EPSILON);
        assert_eq!(parsed.2, Some(461.15));
    }

    #[test]
    fn converts_quicktime_epoch_to_rfc3339() {
        assert_eq!(
            quicktime_seconds_to_rfc3339(2_082_844_800),
            Some("1970-01-01T00:00:00Z".to_string())
        );
    }

    #[test]
    fn normalizes_capture_dates() {
        assert_eq!(
            normalize_quicktime_datetime("2026-07-11T05:41:35+0800"),
            Some("2026-07-11T05:41:35+08:00".to_string())
        );
        assert_eq!(
            normalize_exif_datetime(
                "2026:02:09 10:41:26",
                Some(&"+08:00".to_string())
            ),
            Some("2026-02-09T10:41:26+08:00".to_string())
        );
    }

    #[test]
    fn parses_quicktime_indexed_metadata_without_fullbox_header() {
        let key = b"com.apple.quicktime.make";
        let mut keys = vec![0, 0, 0, 0, 0, 0, 0, 1];
        keys.extend_from_slice(&((key.len() + 8) as u32).to_be_bytes());
        keys.extend_from_slice(b"mdta");
        keys.extend_from_slice(key);

        let mut data = vec![0, 0, 0, 1, 0, 0, 0, 0];
        data.extend_from_slice(b"Apple");
        let item = atom(1u32.to_be_bytes(), &atom(*b"data", &data));
        let mut meta = atom(*b"hdlr", &[0; 24]);
        meta.extend_from_slice(&atom(*b"keys", &keys));
        meta.extend_from_slice(&atom(*b"ilst", &item));

        let mut parsed = HashMap::new();
        parse_metadata_tree(&atom(*b"meta", &meta), &mut parsed);
        assert_eq!(parsed.get("make").map(String::as_str), Some("Apple"));
    }
}
