use std::ffi::{c_char, CStr, CString};

/// Parse media bytes and return a JSON object owned by Rust.
/// The caller must release the returned string with `metaprobe_free_string`.
#[no_mangle]
pub unsafe extern "C" fn metaprobe_extract_json(
    data: *const u8,
    data_len: usize,
    filename: *const c_char,
) -> *mut c_char {
    if data.is_null() || filename.is_null() {
        return std::ptr::null_mut();
    }

    let bytes = std::slice::from_raw_parts(data, data_len);
    let filename = match CStr::from_ptr(filename).to_str() {
        Ok(value) => value,
        Err(_) => return std::ptr::null_mut(),
    };

    let result = match metaprobe_core::extract_from_bytes(bytes, filename) {
        Ok(meta) => serde_json::to_string(&meta),
        Err(error) => serde_json::to_string(&serde_json::json!({
            "error": error.to_string()
        })),
    };

    match result.ok().and_then(|json| CString::new(json).ok()) {
        Some(value) => value.into_raw(),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn metaprobe_free_string(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}
