use serde_json::{json, Value};
use std::panic;

// 导入宿主函数（名称与宿主注册一致）
extern "C" {
    fn host_web_search(query_ptr: u32, query_len: u32, result_ptr: u32) -> i32;
}

/// 将结果写入输入指针指向的缓冲区（复用输入内存）
#[no_mangle]
pub extern "C" fn web_search(input_ptr: *mut u8) -> *mut u8 {
    let result = panic::catch_unwind(|| {
        // 1. 读取查询参数
        let len = unsafe { *(input_ptr as *const u32) } as usize;
        let data_ptr = unsafe { input_ptr.add(4) };
        let data_slice = unsafe { std::slice::from_raw_parts(data_ptr, len) };
        let query = String::from_utf8_lossy(data_slice).to_string();
        eprintln!("WASM received query: {}", query);

        // 2. 临时缓冲区位置（输入缓冲区之后）
        let temp_result_ptr = unsafe { input_ptr.add(4 + len) };
        
        // 3. 调用宿主函数
        let ret = unsafe {
            host_web_search(
                data_ptr as u32,
                len as u32,
                temp_result_ptr as u32,
            )
        };
        
        if ret != 0 {
            let err_msg = json!({ "result": "error", "error": "host function failed" }).to_string();
            let err_bytes = err_msg.as_bytes();
            let err_len = err_bytes.len() as u32;
            unsafe {
                std::ptr::copy_nonoverlapping(&err_len as *const u32 as *const u8, input_ptr, 4);
                std::ptr::copy_nonoverlapping(err_bytes.as_ptr(), input_ptr.add(4), err_bytes.len());
            }
            return Err(());
        }
        
        // 4. 读取宿主返回的结果
        let result_len = unsafe { *(temp_result_ptr as *const u32) } as usize;
        let result_data_ptr = unsafe { temp_result_ptr.add(4) };
        let result_data_slice = unsafe { std::slice::from_raw_parts(result_data_ptr, result_len) };
        let result_str = String::from_utf8_lossy(result_data_slice).to_string();
        
        // 5. 将结果写回输入缓冲区
        let result_bytes = result_str.as_bytes();
        let result_len = result_bytes.len() as u32;
        unsafe {
            std::ptr::copy_nonoverlapping(&result_len as *const u32 as *const u8, input_ptr, 4);
            std::ptr::copy_nonoverlapping(result_bytes.as_ptr(), input_ptr.add(4), result_bytes.len());
        }
        
        Ok::<_, ()>(())
    });

    // 处理 panic
    match result {
        Ok(Ok(())) => {}
        Ok(Err(())) => {}
        Err(_) => {
            eprintln!("WASM panic");
            let err_msg = json!({ "result": "error", "error": "panic occurred" }).to_string();
            let err_bytes = err_msg.as_bytes();
            let err_len = err_bytes.len() as u32;
            unsafe {
                std::ptr::copy_nonoverlapping(&err_len as *const u32 as *const u8, input_ptr, 4);
                std::ptr::copy_nonoverlapping(err_bytes.as_ptr(), input_ptr.add(4), err_bytes.len());
            }
        }
    }

    input_ptr
}

/// 原有的 echo_text 函数
#[no_mangle]
pub extern "C" fn echo_text(input_ptr: *mut u8) -> *mut u8 {
    let result = panic::catch_unwind(|| {
        let len = unsafe { *(input_ptr as *const u32) } as usize;
        let data_ptr = unsafe { input_ptr.add(4) };
        let data_slice = unsafe { std::slice::from_raw_parts(data_ptr, len) };
        let input_str = String::from_utf8_lossy(data_slice).to_string();
        eprintln!("WASM received: {}", input_str);

        let params: Value = serde_json::from_str(&input_str)?;
        let text = params.get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let response = json!({ "result": "success", "echo": text });

        let resp_str = response.to_string();
        let resp_bytes = resp_str.as_bytes();
        let resp_len = resp_bytes.len() as u32;

        unsafe {
            std::ptr::copy_nonoverlapping(&resp_len as *const u32 as *const u8, input_ptr, 4);
            std::ptr::copy_nonoverlapping(resp_bytes.as_ptr(), input_ptr.add(4), resp_bytes.len());
        }

        Ok::<_, serde_json::Error>(())
    }).unwrap_or_else(|_| {
        let err_msg = json!({ "result": "error", "error": "panic occurred" }).to_string();
        let err_bytes = err_msg.as_bytes();
        let err_len = err_bytes.len() as u32;
        unsafe {
            std::ptr::copy_nonoverlapping(&err_len as *const u32 as *const u8, input_ptr, 4);
            std::ptr::copy_nonoverlapping(err_bytes.as_ptr(), input_ptr.add(4), err_bytes.len());
        }
        Ok(())
    });

    input_ptr
}