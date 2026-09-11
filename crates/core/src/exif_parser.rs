use crate::format::ImageFormat;
use std::collections::HashMap;
use std::io::Cursor;

/// Parse EXIF data from raw file bytes, returning key-value pairs as strings.
pub fn parse_exif(data: &[u8], format: ImageFormat) -> HashMap<String, String> {
    let mut map = HashMap::new();

    let reader = create_exif_reader(data, format);

    let result = exif::Reader::new()
        .continue_on_error(true)
        .read_from_container(&mut Cursor::new(reader));

    let exif = match result {
        Ok(e) => e,
        Err(e) => match e.distill_partial_result(|_| {}) {
            Ok(e) => e,
            Err(_) => return map,
        },
    };

    for field in exif.fields() {
        let tag_name = format!("{}", field.tag);
        let value = format_exif_value(field);
        if !value.is_empty() {
            map.insert(tag_name, value);
        }
    }

    normalize_exif_map(&mut map);

    map
}

pub(crate) fn normalize_exif_map(map: &mut HashMap<String, String>) {
    normalize_gps_keys(map);
    add_normalized_aliases(map);
}

fn create_exif_reader(data: &[u8], _format: ImageFormat) -> &[u8] {
    data
}

fn format_exif_value(field: &exif::Field) -> String {
    match &field.value {
        exif::Value::Undefined(bytes, _) if field.tag == exif::Tag::UserComment => {
            format_user_comment(bytes)
        }
        exif::Value::Byte(bytes) if bytes.len() > 128 => format_binary_value(bytes),
        exif::Value::Undefined(bytes, _) if bytes.len() > 128 => format_binary_value(bytes),
        exif::Value::Ascii(values) => format_ascii_value(values),
        exif::Value::Rational(rats) => rats
            .iter()
            .map(|r| {
                if r.denom == 0 {
                    "0".to_string()
                } else {
                    format!("{}", r.num as f64 / r.denom as f64)
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
        exif::Value::SRational(rats) => rats
            .iter()
            .map(|r| {
                if r.denom == 0 {
                    "0".to_string()
                } else {
                    format!("{}", r.num as f64 / r.denom as f64)
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
        _ => field.display_value().with_unit(field).to_string(),
    }
}

fn format_user_comment(bytes: &[u8]) -> String {
    let payload = if bytes.starts_with(b"ASCII\0\0\0") {
        &bytes[8..]
    } else if bytes.starts_with(b"UNICODE\0") {
        let utf16 = &bytes[8..];
        let values = utf16
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        return String::from_utf16_lossy(&values)
            .trim_matches(['\0', ' '])
            .to_string();
    } else {
        bytes
    };
    String::from_utf8_lossy(payload)
        .trim_matches(['\0', ' '])
        .to_string()
}

fn format_ascii_value(values: &[Vec<u8>]) -> String {
    values
        .iter()
        .filter_map(|value| {
            let text = String::from_utf8_lossy(value)
                .trim_matches('\0')
                .trim()
                .to_string();
            (!text.is_empty()).then_some(text)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_binary_value(bytes: &[u8]) -> String {
    let preview_len = bytes.len().min(32);
    format!(
        "0x{}… ({} bytes)",
        hex::encode(&bytes[..preview_len]),
        bytes.len()
    )
}

fn normalize_gps_keys(map: &mut HashMap<String, String>) {
    let keys = ["GPSLatitude", "GPSLongitude", "GPSAltitude"];
    for expected in &keys {
        if !map.contains_key(*expected) {
            let lower = expected.to_lowercase();
            let alt_key = map.keys().find(|k| k.to_lowercase() == lower).cloned();
            if let Some(key) = alt_key {
                if let Some(val) = map.remove(&key) {
                    map.insert(expected.to_string(), val);
                }
            }
        }
    }

    for expected in &keys {
        if let Some(value) = map.get(*expected).cloned() {
            map.insert(format!("{expected}Raw"), value);
        }
    }

    normalize_coordinate(map, "GPSLatitude", "GPSLatitudeRef");
    normalize_coordinate(map, "GPSLongitude", "GPSLongitudeRef");
    if let Some(value) = map.get("GPSAltitude").cloned() {
        if let Some(altitude) = first_number(&value) {
            let sign = map
                .get("GPSAltitudeRef")
                .map(|reference| reference.trim() == "1")
                .unwrap_or(false);
            map.insert(
                "GPSAltitude".to_string(),
                if sign {
                    (-altitude).to_string()
                } else {
                    altitude.to_string()
                },
            );
        }
    }
}

fn add_normalized_aliases(map: &mut HashMap<String, String>) {
    if let Some(latitude) = map.get("GPSLatitude").cloned() {
        map.insert("latitude".to_string(), latitude);
    }
    if let Some(longitude) = map.get("GPSLongitude").cloned() {
        map.insert("longitude".to_string(), longitude);
    }
    if let Some(altitude) = map.get("GPSAltitude").cloned() {
        map.insert("altitude".to_string(), altitude);
    }

    add_date_aliases(
        map,
        "DateTimeOriginal",
        "OffsetTimeOriginal",
        &["DateTimeOriginalISO", "CreateDate"],
    );
    add_date_aliases(
        map,
        "DateTimeDigitized",
        "OffsetTimeDigitized",
        &["DateTimeDigitizedISO"],
    );
    add_date_aliases(
        map,
        "DateTime",
        "OffsetTime",
        &["DateTimeISO", "ModifyDate"],
    );
}

fn add_date_aliases(
    map: &mut HashMap<String, String>,
    source_key: &str,
    offset_key: &str,
    aliases: &[&str],
) {
    let Some(value) = map.get(source_key).cloned() else {
        return;
    };
    let Some(normalized) = normalize_datetime(&value, map.get(offset_key)) else {
        return;
    };
    for alias in aliases {
        map.insert((*alias).to_string(), normalized.clone());
    }
}

fn normalize_datetime(value: &str, offset: Option<&String>) -> Option<String> {
    let value = value.trim().trim_matches('"');
    let (date, time) = value.split_once(' ')?;
    let date = date.replace(':', "-");
    if date.len() != 10 || time.len() < 8 {
        return None;
    }
    let timezone = offset
        .map(|value| value.trim().trim_matches('"'))
        .filter(|value| value.len() == 6 && (value.starts_with('+') || value.starts_with('-')))
        .unwrap_or("Z");
    Some(format!("{}T{}{}", date, &time[..8], timezone))
}

fn normalize_coordinate(map: &mut HashMap<String, String>, key: &str, reference_key: &str) {
    let Some(value) = map.get(key).cloned() else {
        return;
    };
    let values: Vec<f64> = value
        .split(',')
        .filter_map(first_number)
        .collect();
    let Some(degrees) = values.first().copied() else {
        return;
    };
    let minutes = values.get(1).copied().unwrap_or(0.0);
    let seconds = values.get(2).copied().unwrap_or(0.0);
    let decimal = degrees + minutes / 60.0 + seconds / 3600.0;
    let negative = map
        .get(reference_key)
        .map(|reference| {
            let reference = reference.trim().to_ascii_uppercase();
            reference == "S"
                || reference == "W"
                || reference.contains("SOUTH")
                || reference.contains("WEST")
        })
        .unwrap_or(false);
    map.insert(
        key.to_string(),
        if negative {
            (-decimal).to_string()
        } else {
            decimal.to_string()
        },
    );
}

fn first_number(value: &str) -> Option<f64> {
    value
        .split_whitespace()
        .next()
        .unwrap_or(value)
        .trim_matches(|character: char| {
            !character.is_ascii_digit() && character != '.' && character != '-'
        })
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::format_user_comment;

    #[test]
    fn trims_empty_padded_user_comment() {
        assert_eq!(format_user_comment(&[0; 256]), "");
        assert_eq!(format_user_comment(b"ASCII\0\0\0hello\0  "), "hello");
    }
}
