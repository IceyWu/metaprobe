use crate::format::ImageFormat;

/// Image dimensions (width, height) in pixels.
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

/// Extract image dimensions from raw file bytes.
pub fn get_dimensions(data: &[u8], format: ImageFormat) -> Option<Dimensions> {
    let dimensions = match format {
        ImageFormat::Jpeg => get_jpeg_dimensions(data),
        ImageFormat::Png => get_png_dimensions(data),
        ImageFormat::Gif => get_gif_dimensions(data),
        ImageFormat::Bmp => get_bmp_dimensions(data),
        ImageFormat::Tiff => get_tiff_dimensions(data),
        ImageFormat::Webp => get_webp_dimensions(data),
        ImageFormat::Ico => get_ico_dimensions(data),
        ImageFormat::Qoi => get_qoi_dimensions(data),
        ImageFormat::Svg => get_svg_dimensions(data),
        ImageFormat::Heic | ImageFormat::Avif | ImageFormat::Cr3 => get_isobmff_dimensions(data),
        ImageFormat::Jpeg2000 | ImageFormat::Jxl => Some(Dimensions { width: 0, height: 0 }),
        ImageFormat::Mov
        | ImageFormat::Mp4
        | ImageFormat::Avi
        | ImageFormat::Webm
        | ImageFormat::Mkv
        | ImageFormat::Flv
        | ImageFormat::MpegTs
        | ImageFormat::Mpeg
        | ImageFormat::Ogg => None,
    };

    // The container signature is still authoritative even when a format's
    // optional dimension block is unavailable in a metadata-only slice.
    // Returning zero means callers can inspect format/EXIF/ICC instead of
    // being rejected solely because a decoder is not bundled.
    dimensions.or_else(|| (!format.is_video()).then_some(Dimensions { width: 0, height: 0 }))
}

fn get_gif_dimensions(data: &[u8]) -> Option<Dimensions> {
    (data.len() >= 10 && matches!(&data[..6], b"GIF87a" | b"GIF89a")).then(|| Dimensions {
        width: u16::from_le_bytes([data[6], data[7]]) as u32,
        height: u16::from_le_bytes([data[8], data[9]]) as u32,
    })
}

fn get_bmp_dimensions(data: &[u8]) -> Option<Dimensions> {
    if data.len() < 26 || &data[..2] != b"BM" {
        return None;
    }
    let width = i32::from_le_bytes(data[18..22].try_into().ok()?).unsigned_abs();
    let height = i32::from_le_bytes(data[22..26].try_into().ok()?).unsigned_abs();
    Some(Dimensions { width, height })
}

fn get_tiff_dimensions(data: &[u8]) -> Option<Dimensions> {
    let exif = crate::exif_parser::parse_exif(data, ImageFormat::Tiff);
    let width = ["ImageWidth", "PixelXDimension"]
        .iter()
        .find_map(|key| exif.get(*key).and_then(|value| parse_first_u32(value)))?;
    let height = ["ImageLength", "PixelYDimension"]
        .iter()
        .find_map(|key| exif.get(*key).and_then(|value| parse_first_u32(value)))?;
    Some(Dimensions { width, height })
}

fn parse_first_u32(value: &str) -> Option<u32> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| !part.is_empty())
        .and_then(|part| part.parse().ok())
}

fn get_ico_dimensions(data: &[u8]) -> Option<Dimensions> {
    if data.len() < 22 || data[..2] != [0, 0] || !matches!(&data[2..4], [1, 0] | [2, 0]) {
        return None;
    }
    let width = if data[6] == 0 { 256 } else { data[6] as u32 };
    let height = if data[7] == 0 { 256 } else { data[7] as u32 };
    Some(Dimensions { width, height })
}

fn get_qoi_dimensions(data: &[u8]) -> Option<Dimensions> {
    (data.len() >= 14 && &data[..4] == b"qoif").then(|| Dimensions {
        width: u32::from_be_bytes(data[4..8].try_into().unwrap()),
        height: u32::from_be_bytes(data[8..12].try_into().unwrap()),
    })
}

fn get_svg_dimensions(data: &[u8]) -> Option<Dimensions> {
    let source = String::from_utf8_lossy(&data[..data.len().min(4096)]);
    let svg_start = source.find("<svg")?;
    let tag = &source[svg_start..source.find('>').unwrap_or(source.len())];
    let width = parse_svg_length(tag, "width");
    let height = parse_svg_length(tag, "height");
    if let (Some(width), Some(height)) = (width, height) {
        return Some(Dimensions { width, height });
    }
    let view_box = tag.split_once("viewBox")?.1;
    let values = view_box
        .split(|character: char| character == '"' || character == '\'' || character == '=' || character.is_ascii_whitespace())
        .filter_map(|part| part.parse::<f64>().ok())
        .collect::<Vec<_>>();
    (values.len() >= 4 && values[2] > 0.0 && values[3] > 0.0).then(|| Dimensions {
        width: values[2].round() as u32,
        height: values[3].round() as u32,
    })
}

fn parse_svg_length(tag: &str, attribute: &str) -> Option<u32> {
    let value = tag.split_once(&format!("{attribute}="))?.1;
    let value = value.trim_start_matches(['"', '\'', ' ']);
    let number = value
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '.')
        .collect::<String>();
    number.parse::<f64>().ok().filter(|value| *value > 0.0).map(|value| value.round() as u32)
}

fn get_jpeg_dimensions(data: &[u8]) -> Option<Dimensions> {
    let mut pos = 2; // Skip SOI

    while pos + 4 < data.len() {
        if data[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let marker = data[pos + 1];

        // SOF markers (SOF0-SOF15, excluding DHT=0xC4, JPG=0xC8, DAC=0xCC)
        if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF)
            && pos + 9 < data.len()
        {
            let height = u16::from_be_bytes([data[pos + 5], data[pos + 6]]) as u32;
            let width = u16::from_be_bytes([data[pos + 7], data[pos + 8]]) as u32;
            return Some(Dimensions { width, height });
        }

        if marker == 0xD9 || marker == 0xDA {
            break;
        }

        // Skip marker segment
        if pos + 4 <= data.len() {
            let seg_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
            if seg_len < 2 {
                break;
            }
            pos += 2 + seg_len;
        } else {
            break;
        }
    }

    None
}

fn get_png_dimensions(data: &[u8]) -> Option<Dimensions> {
    // IHDR chunk starts at offset 8 (after PNG signature)
    // 4 bytes length + 4 bytes "IHDR" + 4 bytes width + 4 bytes height
    if data.len() < 24 {
        return None;
    }
    if &data[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    Some(Dimensions { width, height })
}

fn get_webp_dimensions(data: &[u8]) -> Option<Dimensions> {
    if data.len() < 16 {
        return None;
    }

    let mut pos = 12;
    while pos + 8 <= data.len() {
        let fourcc = &data[pos..pos + 4];
        let chunk_size =
            u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
                as usize;
        let payload_start = pos + 8;

        // VP8X extended format
        if fourcc == b"VP8X" && payload_start + 10 <= data.len() {
            let w = (data[payload_start + 4] as u32)
                | ((data[payload_start + 5] as u32) << 8)
                | ((data[payload_start + 6] as u32) << 16);
            let h = (data[payload_start + 7] as u32)
                | ((data[payload_start + 8] as u32) << 8)
                | ((data[payload_start + 9] as u32) << 16);
            return Some(Dimensions {
                width: w + 1,
                height: h + 1,
            });
        }

        // VP8 lossy
        if fourcc == b"VP8 " && payload_start + 10 <= data.len() {
            // VP8 bitstream starts with a frame tag
            if data[payload_start + 3..payload_start + 6] == [0x9D, 0x01, 0x2A] {
                let width =
                    u16::from_le_bytes([data[payload_start + 6], data[payload_start + 7]]) & 0x3FFF;
                let height =
                    u16::from_le_bytes([data[payload_start + 8], data[payload_start + 9]]) & 0x3FFF;
                return Some(Dimensions {
                    width: width as u32,
                    height: height as u32,
                });
            }
        }

        // VP8L lossless
        if fourcc == b"VP8L"
            && payload_start + 5 <= data.len()
            && data[payload_start] == 0x2F
        {
            let bits = u32::from_le_bytes([
                data[payload_start + 1],
                data[payload_start + 2],
                data[payload_start + 3],
                data[payload_start + 4],
            ]);
            let width = (bits & 0x3FFF) + 1;
            let height = ((bits >> 14) & 0x3FFF) + 1;
            return Some(Dimensions { width, height });
        }

        let padded_size = (chunk_size + 1) & !1;
        pos += 8 + padded_size;
    }

    None
}

/// Extract dimensions from ISOBMFF container (HEIC/AVIF) via ispe box.
fn get_isobmff_dimensions(data: &[u8]) -> Option<Dimensions> {
    find_ispe_in_boxes(data, 0, data.len())
}

fn find_ispe_in_boxes(data: &[u8], start: usize, end: usize) -> Option<Dimensions> {
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

        // ispe box: 4 bytes version/flags + 4 bytes width + 4 bytes height
        if box_type == b"ispe" && pos + header_size + 12 <= pos + actual_size {
            let ispe_data = &data[pos + header_size..pos + actual_size];
            if ispe_data.len() >= 12 {
                let width =
                    u32::from_be_bytes([ispe_data[4], ispe_data[5], ispe_data[6], ispe_data[7]]);
                let height =
                    u32::from_be_bytes([ispe_data[8], ispe_data[9], ispe_data[10], ispe_data[11]]);
                return Some(Dimensions { width, height });
            }
        }

        // Navigate into container boxes
        let container_types: &[&[u8; 4]] = &[b"meta", b"iprp", b"ipco", b"moov", b"trak", b"mdia"];
        if container_types.iter().any(|t| *t == box_type) {
            let child_start = if box_type == b"meta" {
                pos + header_size + 4 // meta has version/flags
            } else {
                pos + header_size
            };
            if let Some(dims) = find_ispe_in_boxes(data, child_start, pos + actual_size) {
                return Some(dims);
            }
        }

        pos += actual_size;
    }

    None
}
