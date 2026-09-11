/// Media formats detected from content signatures.
///
/// File extensions are deliberately only a fallback hint. `detect_format` is
/// the primary path so a renamed file can still be parsed safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    Bmp,
    Tiff,
    Webp,
    Ico,
    Qoi,
    Svg,
    Jpeg2000,
    Jxl,
    Heic,
    Avif,
    Cr3,
    Mov,
    Mp4,
    Avi,
    Webm,
    Mkv,
    Flv,
    MpegTs,
    Mpeg,
    Ogg,
}

impl ImageFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpeg",
            ImageFormat::Png => "png",
            ImageFormat::Gif => "gif",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Tiff => "tiff",
            ImageFormat::Webp => "webp",
            ImageFormat::Ico => "ico",
            ImageFormat::Qoi => "qoi",
            ImageFormat::Svg => "svg",
            ImageFormat::Jpeg2000 => "jpeg2000",
            ImageFormat::Jxl => "jxl",
            ImageFormat::Heic => "heic",
            ImageFormat::Avif => "avif",
            ImageFormat::Cr3 => "cr3",
            ImageFormat::Mov => "mov",
            ImageFormat::Mp4 => "mp4",
            ImageFormat::Avi => "avi",
            ImageFormat::Webm => "webm",
            ImageFormat::Mkv => "mkv",
            ImageFormat::Flv => "flv",
            ImageFormat::MpegTs => "mpeg-ts",
            ImageFormat::Mpeg => "mpeg",
            ImageFormat::Ogg => "ogg",
        }
    }

    pub fn is_video(&self) -> bool {
        matches!(
            self,
            ImageFormat::Mov
                | ImageFormat::Mp4
                | ImageFormat::Avi
                | ImageFormat::Webm
                | ImageFormat::Mkv
                | ImageFormat::Flv
                | ImageFormat::MpegTs
                | ImageFormat::Mpeg
                | ImageFormat::Ogg
        )
    }
}

/// Detect a media format from magic bytes and lightweight container headers.
pub fn detect_format(data: &[u8]) -> Option<ImageFormat> {
    if data.len() >= 3 && data[..3] == [0xFF, 0xD8, 0xFF] {
        return Some(ImageFormat::Jpeg);
    }
    if data.len() >= 8 && data[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return Some(ImageFormat::Png);
    }
    if data.len() >= 6 && matches!(&data[..6], b"GIF87a" | b"GIF89a") {
        return Some(ImageFormat::Gif);
    }
    if data.len() >= 2 && &data[..2] == b"BM" {
        return Some(ImageFormat::Bmp);
    }
    if data.len() >= 4
        && matches!(&data[..4], b"II*\0" | b"MM\0*" | b"II+\0" | b"MM\0+")
    {
        return Some(ImageFormat::Tiff);
    }
    if data.len() >= 4 && &data[..4] == b"qoif" {
        return Some(ImageFormat::Qoi);
    }
    if data.len() >= 6
        && data[..2] == [0, 0]
        && matches!(&data[2..4], [1, 0] | [2, 0])
        && u16::from_le_bytes([data[4], data[5]]) > 0
    {
        return Some(ImageFormat::Ico);
    }
    if data.len() >= 12
        && data[..12] == [0, 0, 0, 12, b'j', b'P', b' ', b' ', 0x0D, 0x0A, 0x87, 0x0A]
    {
        return Some(ImageFormat::Jpeg2000);
    }
    if data.len() >= 2 && data[..2] == [0xFF, 0x0A] {
        return Some(ImageFormat::Jxl);
    }
    if data.len() >= 12
        && data[..12] == [0, 0, 0, 12, b'J', b'X', b'L', b' ', 0x0D, 0x0A, 0x87, 0x0A]
    {
        return Some(ImageFormat::Jxl);
    }
    if looks_like_svg(data) {
        return Some(ImageFormat::Svg);
    }

    if data.len() >= 12 && &data[..4] == b"RIFF" {
        if &data[8..12] == b"WEBP" {
            return Some(ImageFormat::Webp);
        }
        if &data[8..12] == b"AVI " {
            return Some(ImageFormat::Avi);
        }
    }

    if data.len() >= 12 && &data[4..8] == b"ftyp" {
        return detect_isobmff(data);
    }
    if data.len() >= 4 && data[..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        let probe = &data[..data.len().min(4096)];
        return if probe.windows(4).any(|window| window == b"webm") {
            Some(ImageFormat::Webm)
        } else {
            Some(ImageFormat::Mkv)
        };
    }
    if data.len() >= 3 && &data[..3] == b"FLV" {
        return Some(ImageFormat::Flv);
    }
    if data.len() >= 4 && &data[..4] == b"OggS" {
        return Some(ImageFormat::Ogg);
    }
    if looks_like_mpeg_ts(data) {
        return Some(ImageFormat::MpegTs);
    }
    if data.len() >= 4 && (data[..4] == [0, 0, 1, 0xBA] || data[..4] == [0, 0, 1, 0xB3]) {
        return Some(ImageFormat::Mpeg);
    }

    None
}

fn detect_isobmff(data: &[u8]) -> Option<ImageFormat> {
    let box_size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let box_end = if box_size == 0 {
        data.len()
    } else {
        box_size.min(data.len())
    };
    let brands = (8..box_end.saturating_sub(3))
        .step_by(4)
        .map(|offset| &data[offset..offset + 4]);

    let mut has_avif_brand = false;
    let mut has_heif_brand = false;
    let mut has_mp4_brand = false;
    for brand in brands {
        if brand == b"qt  " {
            return Some(ImageFormat::Mov);
        }
        if matches!(brand, b"avif" | b"avis") {
            has_avif_brand = true;
        }
        if matches!(brand, b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" | b"mif1" | b"msf1") {
            has_heif_brand = true;
        }
        if matches!(brand, b"crx " | b"cr3 ") {
            return Some(ImageFormat::Cr3);
        }
        if matches!(
            brand,
            b"mp41"
                | b"mp42"
                | b"isom"
                | b"iso2"
                | b"iso5"
                | b"iso6"
                | b"avc1"
                | b"mmp4"
                | b"M4V "
                | b"3gp4"
                | b"3gp5"
                | b"3g2a"
                | b"F4V "
        ) {
            has_mp4_brand = true;
        }
    }

    // An unknown ISO-BMFF brand is still a useful video/container result when
    // it contains a movie box. This keeps extensions from becoming a hard limit.
    if has_avif_brand {
        Some(ImageFormat::Avif)
    } else if has_heif_brand {
        Some(ImageFormat::Heic)
    } else if has_mp4_brand || data.windows(4).any(|window| window == b"moov") {
        Some(ImageFormat::Mp4)
    } else {
        None
    }
}

fn looks_like_svg(data: &[u8]) -> bool {
    let probe = String::from_utf8_lossy(&data[..data.len().min(4096)]);
    let probe = probe.trim_start_matches('\u{FEFF}').trim_start();
    if probe.starts_with("<?xml") {
        return probe.find("<svg").is_some();
    }
    probe.starts_with("<svg")
}

fn looks_like_mpeg_ts(data: &[u8]) -> bool {
    data.len() >= 377
        && data[0] == 0x47
        && (data[188] == 0x47 || data[192] == 0x47)
        && (data[376] == 0x47 || data.len() > 384 && data[384] == 0x47)
}

/// Check whether a path extension matches a known format. This is only used
/// after content detection fails, so unknown extensions remain supported.
pub fn format_from_extension(path: &str) -> Option<ImageFormat> {
    let extension = path.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match extension.as_str() {
        "jpg" | "jpeg" | "jpe" => ImageFormat::Jpeg,
        "png" => ImageFormat::Png,
        "gif" => ImageFormat::Gif,
        "bmp" | "dib" => ImageFormat::Bmp,
        "tif" | "tiff" | "dng" | "cr2" | "nef" | "arw" | "orf" | "rw2" => ImageFormat::Tiff,
        "webp" => ImageFormat::Webp,
        "ico" | "cur" => ImageFormat::Ico,
        "qoi" => ImageFormat::Qoi,
        "svg" | "svgz" => ImageFormat::Svg,
        "jp2" | "j2k" | "jpf" | "jpx" => ImageFormat::Jpeg2000,
        "jxl" => ImageFormat::Jxl,
        "heic" | "heif" => ImageFormat::Heic,
        "avif" => ImageFormat::Avif,
        "cr3" => ImageFormat::Cr3,
        "mov" => ImageFormat::Mov,
        "mp4" | "m4v" | "m4a" | "3gp" | "3g2" | "f4v" => ImageFormat::Mp4,
        "avi" => ImageFormat::Avi,
        "webm" => ImageFormat::Webm,
        "mkv" | "mk3d" | "mka" => ImageFormat::Mkv,
        "flv" => ImageFormat::Flv,
        "ts" | "mts" | "m2ts" => ImageFormat::MpegTs,
        "mpg" | "mpeg" | "mpe" | "vob" => ImageFormat::Mpeg,
        "ogv" | "ogg" => ImageFormat::Ogg,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{detect_format, format_from_extension, ImageFormat};

    #[test]
    fn detects_quicktime_and_mp4_brands() {
        let mov = *b"\0\0\0\x10ftypqt  \0\0\0\0";
        let mp4 = *b"\0\0\0\x10ftypisom\0\0\0\0";
        assert_eq!(detect_format(&mov), Some(ImageFormat::Mov));
        assert_eq!(detect_format(&mp4), Some(ImageFormat::Mp4));
    }

    #[test]
    fn detects_common_formats_without_extensions() {
        assert_eq!(detect_format(b"GIF89a"), Some(ImageFormat::Gif));
        assert_eq!(detect_format(b"BM\0\0"), Some(ImageFormat::Bmp));
        assert_eq!(detect_format(b"qoif\0\0\0\0"), Some(ImageFormat::Qoi));
        assert_eq!(detect_format(b"<svg viewBox=\"0 0 1 1\"></svg>"), Some(ImageFormat::Svg));
    }

    #[test]
    fn detects_compatible_brand_when_major_brand_is_generic() {
        let avif = *b"\0\0\0\x18ftypmif1\0\0\0\0avifmif1";
        assert_eq!(detect_format(&avif), Some(ImageFormat::Avif));
    }

    #[test]
    fn detects_video_extensions_case_insensitively() {
        assert_eq!(format_from_extension("movie.MOV"), Some(ImageFormat::Mov));
        assert_eq!(format_from_extension("movie.M4V"), Some(ImageFormat::Mp4));
    }
}
