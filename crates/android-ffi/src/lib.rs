use jni::objects::{JByteArray, JObject, JString};
use jni::sys::jstring;
use jni::JNIEnv;

#[no_mangle]
pub extern "system" fn Java_expo_modules_metaprobe_MetaprobeModule_nativeExtract(
    mut env: JNIEnv<'_>,
    _this: JObject<'_>,
    bytes: JByteArray<'_>,
    filename: JString<'_>,
) -> jstring {
    let result = (|| {
        let bytes = env
            .convert_byte_array(bytes)
            .map_err(|error| error.to_string())?;
        let filename = env
            .get_string(&filename)
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .into_owned();
        let metadata = metaprobe_core::extract_from_bytes_without_hash(&bytes, &filename)
            .map_err(|error| error.to_string())?;
        serde_json::to_string(&metadata).map_err(|error| error.to_string())
    })();

    let value = match result {
        Ok(json) => json,
        Err(message) => match serde_json::to_string(&serde_json::json!({ "error": message })) {
            Ok(json) => json,
            Err(_) => r#"{"error":"metadata extraction failed"}"#.to_string(),
        },
    };

    env.new_string(value)
        .map(|string| string.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
