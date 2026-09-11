use crate::format::ImageFormat;
use flate2::read::ZlibDecoder;
use std::collections::HashMap;
use std::io::Read;

/// Extract the raw ICC profile bytes from image data.
pub fn extract_icc_profile(data: &[u8], format: ImageFormat) -> Option<Vec<u8>> {
    match format {
        ImageFormat::Jpeg => extract_icc_jpeg(data),
        ImageFormat::Png => extract_icc_png(data),
        ImageFormat::Webp => extract_icc_webp(data),
        ImageFormat::Heic | ImageFormat::Avif => extract_icc_isobmff(data),
        ImageFormat::Gif
        | ImageFormat::Bmp
        | ImageFormat::Tiff
        | ImageFormat::Ico
        | ImageFormat::Qoi
        | ImageFormat::Svg
        | ImageFormat::Jpeg2000
        | ImageFormat::Jxl
        | ImageFormat::Cr3
        | ImageFormat::Mov
        | ImageFormat::Mp4
        | ImageFormat::Avi
        | ImageFormat::Webm
        | ImageFormat::Mkv
        | ImageFormat::Flv
        | ImageFormat::MpegTs
        | ImageFormat::Mpeg
        | ImageFormat::Ogg => None,
    }
}

/// Identify color space from a raw ICC profile.
pub fn detect_color_space(icc: &[u8]) -> String {
    if icc.len() < 132 {
        return "Unknown".to_string();
    }

    let desc = extract_icc_description(icc);
    let desc_lower = desc.to_lowercase();

    if desc_lower.contains("srgb") || desc_lower.contains("s rgb") {
        return "sRGB".to_string();
    }
    if desc_lower.contains("adobe rgb") || desc_lower.contains("adobergb") {
        return "AdobeRGB".to_string();
    }
    if desc_lower.contains("display p3")
        || desc_lower.contains("displayp3")
        || desc_lower.contains("p3")
    {
        return "DisplayP3".to_string();
    }
    if desc_lower.contains("prophoto")
        || desc_lower.contains("pro photo")
        || desc_lower.contains("romm")
    {
        return "ProPhotoRGB".to_string();
    }

    "Unknown".to_string()
}

/// Parse ICC profile header and tag table into human-readable key-value pairs.
pub fn parse_icc_metadata(icc: &[u8]) -> HashMap<String, String> {
    let mut map = HashMap::new();

    if icc.len() < 128 {
        return map;
    }

    // Profile size
    let size = u32::from_be_bytes([icc[0], icc[1], icc[2], icc[3]]);
    map.insert("ProfileSize".into(), size.to_string());

    map.insert(
        "ProfileCMMType".into(),
        icc_signature_name(&icc[4..8]).to_string(),
    );

    // Profile version (major.minor.bugfix)
    let major = icc[8];
    let minor_bugfix = icc[9];
    let minor = minor_bugfix >> 4;
    let bugfix = minor_bugfix & 0x0F;
    map.insert(
        "ProfileVersion".into(),
        format!("{}.{}.{}", major, minor, bugfix),
    );

    // Profile/Device class
    let class_sig = &icc[12..16];
    let class_name = match class_sig {
        b"scnr" => "Input",
        b"mntr" => "Monitor",
        b"prtr" => "Printer",
        b"link" => "DeviceLink",
        b"spac" => "ColorSpace",
        b"abst" => "Abstract",
        b"nmcl" => "NamedColor",
        _ => "Unknown",
    };
    map.insert("ProfileClass".into(), class_name.into());

    // Color space of data
    let cs = std::str::from_utf8(&icc[16..20]).unwrap_or("????").trim();
    map.insert("ColorSpaceData".into(), cs.into());

    // Profile connection space
    let pcs = std::str::from_utf8(&icc[20..24]).unwrap_or("????").trim();
    map.insert("ProfileConnectionSpace".into(), pcs.into());

    // Date/time (bytes 24-35): year, month, day, hour, minute, second (each u16 BE)
    if icc.len() >= 36 {
        let year = u16::from_be_bytes([icc[24], icc[25]]);
        let month = u16::from_be_bytes([icc[26], icc[27]]);
        let day = u16::from_be_bytes([icc[28], icc[29]]);
        let hour = u16::from_be_bytes([icc[30], icc[31]]);
        let min = u16::from_be_bytes([icc[32], icc[33]]);
        let sec = u16::from_be_bytes([icc[34], icc[35]]);
        map.insert(
            "ProfileDateTime".into(),
            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                year, month, day, hour, min, sec
            ),
        );
    }

    // File signature (should be 'acsp')
    let sig = std::str::from_utf8(&icc[36..40]).unwrap_or("????");
    map.insert("ProfileFileSignature".into(), sig.into());

    // Primary platform
    let platform_name = icc_signature_name(&icc[40..44]);
    map.insert("PrimaryPlatform".into(), platform_name.into());

    if icc.len() >= 56 {
        map.insert(
            "DeviceManufacturer".into(),
            icc_signature_name(&icc[48..52]).to_string(),
        );
        map.insert(
            "DeviceModel".into(),
            icc_signature_name(&icc[52..56]).to_string(),
        );
    }

    if icc.len() >= 84 {
        map.insert(
            "ProfileCreator".into(),
            icc_signature_name(&icc[80..84]).to_string(),
        );
    }

    // Rendering intent
    if icc.len() >= 68 {
        let intent = u32::from_be_bytes([icc[64], icc[65], icc[66], icc[67]]);
        let intent_name = match intent {
            0 => "Perceptual",
            1 => "Relative Colorimetric",
            2 => "Saturation",
            3 => "Absolute Colorimetric",
            _ => "Unknown",
        };
        map.insert("RenderingIntent".into(), intent_name.into());
    }

    // Profile description from tag table
    let desc = extract_icc_description(icc);
    if !desc.is_empty() {
        map.insert("ProfileDescription".into(), desc);
    }

    // Copyright from tag table
    let copyright = extract_icc_tag_text(icc, b"cprt");
    if !copyright.is_empty() {
        map.insert("Copyright".into(), copyright.clone());
        map.insert("ProfileCopyright".into(), copyright);
    }

    for (name, signature) in [
        ("MediaWhitePoint", b"wtpt"),
        ("RedMatrixColumn", b"rXYZ"),
        ("GreenMatrixColumn", b"gXYZ"),
        ("BlueMatrixColumn", b"bXYZ"),
    ] {
        if let Some(tag) = extract_icc_tag(icc, signature) {
            if let Some(value) = parse_xyz_tag(tag) {
                map.insert(name.into(), value);
            }
        }
    }

    for (name, signature) in [
        ("RedTRC", b"rTRC"),
        ("GreenTRC", b"gTRC"),
        ("BlueTRC", b"bTRC"),
    ] {
        if let Some(tag) = extract_icc_tag(icc, signature) {
            if let Some(value) = parse_curve_tag(tag) {
                map.insert(name.into(), value);
            }
        }
    }

    if let Some(tag) = extract_icc_tag(icc, b"chad") {
        if let Some(value) = parse_matrix_tag(tag) {
            map.insert("ChromaticAdaptation".into(), value);
        }
    }

    map
}

fn icc_signature_name(signature: &[u8]) -> &str {
    match signature {
        b"APPL" => "Apple Computer",
        b"MSFT" => "Microsoft",
        b"ADBE" => "Adobe",
        b"Lino" => "Linotype",
        b"SUNW" => "Sun",
        _ => {
            let value = std::str::from_utf8(signature).unwrap_or("").trim_matches(['\0', ' ']);
            if value.is_empty() { "Unknown" } else { value }
        }
    }
}

fn extract_icc_tag<'a>(icc: &'a [u8], sig_target: &[u8; 4]) -> Option<&'a [u8]> {
    if icc.len() < 132 {
        return None;
    }

    let tag_count = u32::from_be_bytes([icc[128], icc[129], icc[130], icc[131]]) as usize;
    for i in 0..tag_count.min(100) {
        let base = 132 + i * 12;
        if base + 12 > icc.len() {
            break;
        }
        if &icc[base..base + 4] != sig_target {
            continue;
        }
        let offset =
            u32::from_be_bytes([icc[base + 4], icc[base + 5], icc[base + 6], icc[base + 7]])
                as usize;
        let size =
            u32::from_be_bytes([icc[base + 8], icc[base + 9], icc[base + 10], icc[base + 11]])
                as usize;
        return (size > 0 && offset.checked_add(size)? <= icc.len())
            .then(|| &icc[offset..offset + size]);
    }
    None
}

fn parse_xyz_tag(data: &[u8]) -> Option<String> {
    if data.len() < 20 || &data[..4] != b"XYZ " {
        return None;
    }
    let values = [
        read_s15_fixed16(&data[8..12])?,
        read_s15_fixed16(&data[12..16])?,
        read_s15_fixed16(&data[16..20])?,
    ];
    Some(format!(
        "{:.6}, {:.6}, {:.6}",
        values[0], values[1], values[2]
    ))
}

fn parse_curve_tag(data: &[u8]) -> Option<String> {
    if data.len() < 12 {
        return None;
    }
    if &data[..4] == b"para" {
        let function = u16::from_be_bytes(data[8..10].try_into().ok()?);
        let parameter_count = match function {
            0 => 1,
            1 => 3,
            2 => 4,
            3 => 5,
            4 => 7,
            _ => return Some(format!("parametric function {function}")),
        };
        let end = 12usize.checked_add(parameter_count * 4)?;
        if end > data.len() {
            return None;
        }
        let parameters = data[12..end]
            .chunks_exact(4)
            .filter_map(read_s15_fixed16)
            .map(|value| format!("{value:.6}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Some(format!("parametric {function} ({parameters})"));
    }
    if &data[..4] != b"curv" {
        return None;
    }
    let count = u32::from_be_bytes(data[8..12].try_into().ok()?) as usize;
    match count {
        0 => Some("identity".into()),
        1 if data.len() >= 14 => Some(format!(
            "gamma {:.4}",
            u16::from_be_bytes(data[12..14].try_into().ok()?) as f64 / 256.0
        )),
        _ => Some(format!("curve ({} entries)", count)),
    }
}

fn parse_matrix_tag(data: &[u8]) -> Option<String> {
    if data.len() < 44 || &data[..4] != b"sf32" {
        return None;
    }
    let mut values = Vec::with_capacity(9);
    for chunk in data[8..44].chunks_exact(4) {
        values.push(read_s15_fixed16(chunk)?);
    }
    Some(
        values
            .iter()
            .map(|value| format!("{:.6}", value))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

fn read_s15_fixed16(data: &[u8]) -> Option<f64> {
    Some(i32::from_be_bytes(data.try_into().ok()?) as f64 / 65536.0)
}

/// Extract a text-type tag value from the ICC tag table by its 4-byte signature.
fn extract_icc_tag_text(icc: &[u8], sig_target: &[u8; 4]) -> String {
    if icc.len() < 132 {
        return String::new();
    }

    if let Some(tag_data) = extract_icc_tag(icc, sig_target) {
        // Try parsing as mluc or desc first, fall back to plain text.
        let result = parse_desc_tag(tag_data);
        if !result.is_empty() {
            return result;
        }
        // 'text' type (signature "text")
        if tag_data.len() > 8 && &tag_data[0..4] == b"text" {
            return String::from_utf8_lossy(&tag_data[8..])
                .trim_end_matches('\0')
                .to_string();
        }
    }

    String::new()
}

fn extract_icc_description(icc: &[u8]) -> String {
    if icc.len() < 132 {
        return String::new();
    }

    let tag_count = u32::from_be_bytes([icc[128], icc[129], icc[130], icc[131]]) as usize;
    let max_tags = tag_count.min(100);

    for i in 0..max_tags {
        let base = 132 + i * 12;
        if base + 12 > icc.len() {
            break;
        }
        let sig = &icc[base..base + 4];
        if sig == b"desc" {
            let offset =
                u32::from_be_bytes([icc[base + 4], icc[base + 5], icc[base + 6], icc[base + 7]])
                    as usize;
            let size =
                u32::from_be_bytes([icc[base + 8], icc[base + 9], icc[base + 10], icc[base + 11]])
                    as usize;

            if offset + size <= icc.len() && size > 0 {
                return parse_desc_tag(&icc[offset..offset + size]);
            }
        }
    }

    String::new()
}

fn parse_desc_tag(data: &[u8]) -> String {
    if data.len() < 8 {
        return String::new();
    }

    let type_sig = &data[0..4];

    // 'mluc' - Multi Localized Unicode Type
    if type_sig == b"mluc" && data.len() >= 28 {
        let str_len = u32::from_be_bytes([data[20], data[21], data[22], data[23]]) as usize;
        let str_offset = u32::from_be_bytes([data[24], data[25], data[26], data[27]]) as usize;
        if str_offset + str_len <= data.len() {
            let utf16_data = &data[str_offset..str_offset + str_len];
            let chars: Vec<u16> = utf16_data
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            return String::from_utf16_lossy(&chars)
                .trim_end_matches('\0')
                .to_string();
        }
    }

    // 'desc' - Text Description Type
    if type_sig == b"desc" && data.len() >= 12 {
        let str_len = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;
        if str_len > 0 && 12 + str_len <= data.len() {
            return String::from_utf8_lossy(&data[12..12 + str_len])
                .trim_end_matches('\0')
                .to_string();
        }
    }

    String::new()
}

/// Extract ICC profile from JPEG (APP2 marker with "ICC_PROFILE\0" header).
fn extract_icc_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    let mut chunks: Vec<(u8, Vec<u8>)> = Vec::new();
    let mut pos = 2; // Skip SOI (FF D8)

    while pos + 4 < data.len() {
        if data[pos] != 0xFF {
            break;
        }
        let marker = data[pos + 1];
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if pos + 4 > data.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        if seg_len < 2 || pos + 2 + seg_len > data.len() {
            break;
        }

        if marker == 0xE2 && seg_len >= 16 {
            let payload = &data[pos + 4..pos + 2 + seg_len];
            if payload.len() >= 14 && &payload[0..12] == b"ICC_PROFILE\0" {
                let chunk_num = payload[12];
                let chunk_data = payload[14..].to_vec();
                chunks.push((chunk_num, chunk_data));
            }
        }

        pos += 2 + seg_len;
    }

    if chunks.is_empty() {
        return None;
    }

    chunks.sort_by_key(|(num, _)| *num);
    Some(chunks.into_iter().flat_map(|(_, d)| d).collect())
}

/// Extract ICC profile from PNG (iCCP chunk).
fn extract_icc_png(data: &[u8]) -> Option<Vec<u8>> {
    let mut pos = 8;

    while pos + 8 <= data.len() {
        let chunk_len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let chunk_type = &data[pos + 4..pos + 8];

        if pos + 8 + chunk_len + 4 > data.len() {
            break;
        }

        if chunk_type == b"iCCP" {
            let chunk_data = &data[pos + 8..pos + 8 + chunk_len];
            if let Some(null_pos) = chunk_data.iter().position(|&b| b == 0) {
                if null_pos + 2 <= chunk_data.len() {
                    let compressed = &chunk_data[null_pos + 2..];
                    let mut decoder = ZlibDecoder::new(compressed);
                    let mut decompressed = Vec::new();
                    if decoder.read_to_end(&mut decompressed).is_ok() {
                        return Some(decompressed);
                    }
                }
            }
        }

        if chunk_type == b"IDAT" {
            break;
        }

        pos += 8 + chunk_len + 4;
    }

    None
}

/// Extract ICC profile from WEBP (ICCP chunk in RIFF container).
fn extract_icc_webp(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return None;
    }

    let mut pos = 12;
    while pos + 8 <= data.len() {
        let fourcc = &data[pos..pos + 4];
        let chunk_size =
            u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
                as usize;

        if fourcc == b"ICCP" && pos + 8 + chunk_size <= data.len() {
            return Some(data[pos + 8..pos + 8 + chunk_size].to_vec());
        }

        let padded_size = (chunk_size + 1) & !1;
        pos += 8 + padded_size;
    }

    None
}

/// Extract ICC profile from ISOBMFF container (HEIC/AVIF).
fn extract_icc_isobmff(data: &[u8]) -> Option<Vec<u8>> {
    find_icc_in_isobmff_boxes(data, 0, data.len())
}

fn find_icc_in_isobmff_boxes(data: &[u8], start: usize, end: usize) -> Option<Vec<u8>> {
    let mut pos = start;

    while pos + 8 <= end {
        let box_size =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let box_type = &data[pos + 4..pos + 8];

        let actual_size = if box_size == 0 {
            end - pos
        } else if box_size == 1 && pos + 16 <= end {
            u64::from_be_bytes([
                data[pos + 8],
                data[pos + 9],
                data[pos + 10],
                data[pos + 11],
                data[pos + 12],
                data[pos + 13],
                data[pos + 14],
                data[pos + 15],
            ]) as usize
        } else {
            box_size
        };

        if actual_size < 8 || pos + actual_size > end {
            break;
        }

        let header_size = if box_size == 1 { 16 } else { 8 };

        if box_type == b"colr" && pos + header_size + 4 <= pos + actual_size {
            let colr_data = &data[pos + header_size..pos + actual_size];
            if colr_data.len() >= 4 {
                let color_type = &colr_data[0..4];
                if color_type == b"prof" || color_type == b"rICC" {
                    return Some(colr_data[4..].to_vec());
                }
            }
        }

        let container_types: &[&[u8; 4]] = &[b"meta", b"iprp", b"ipco", b"moov", b"trak", b"mdia"];
        if container_types.iter().any(|t| *t == box_type) {
            let child_start = if box_type == b"meta" {
                pos + header_size + 4
            } else {
                pos + header_size
            };
            if let Some(icc) = find_icc_in_isobmff_boxes(data, child_start, pos + actual_size) {
                return Some(icc);
            }
        }

        pos += actual_size;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{icc_signature_name, parse_curve_tag};

    #[test]
    fn normalizes_empty_icc_signatures() {
        assert_eq!(icc_signature_name(&[0, 0, 0, 0]), "Unknown");
    }

    #[test]
    fn parses_parametric_tone_curve() {
        let mut tag = b"para\0\0\0\0\0\0\0\0".to_vec();
        tag.extend_from_slice(&0x0002_6666i32.to_be_bytes());
        assert_eq!(parse_curve_tag(&tag), Some("parametric 0 (2.399994)".into()));
    }
}
