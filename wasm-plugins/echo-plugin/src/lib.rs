use serde_json::{json, Value};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::ffi::CString;
use std::panic;

#[no_mangle]
pub extern "C" fn echo_text(input_ptr: *const c_char) -> *mut c_char {
    let result = panic::catch_unwind(|| {
        let ptr = input_ptr as usize;
        let len = unsafe { *(ptr as *const u32) } as usize;
        let data_ptr = (ptr + 4) as *const u8;
        let data_slice = unsafe { std::slice::from_raw_parts(data_ptr, len) };
        let input_str = String::from_utf8_lossy(data_slice).to_string();
        eprintln!("WASM received: {}", input_str);

        let params: Value = serde_json::from_str(&input_str)
            .unwrap_or_else(|_| json!({}));
        let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");

        json!({ "result": "success", "echo": text })
    }).unwrap_or_else(|_| json!({ "result": "error", "error": "panic occurred" }));

    let result_str = result.to_string();
    let c_str = CString::new(result_str).unwrap();
    c_str.into_raw()
}