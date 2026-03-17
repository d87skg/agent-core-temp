# WASM 宿主函数调用指南

本文档介绍如何在 WASM 插件中调用 agent-core 提供的宿主函数。所有示例使用 Rust 编写 WASM 模块，通过 `wit-bindgen` 或直接导入的方式调用。

## 1. 日志输出

```rust
#[link(wasm_import_module = "host")]
extern {
    fn log(level: u32, ptr: u32, len: u32);
}

fn log_info(msg: &str) {
    log(1, msg.as_ptr() as u32, msg.len() as u32);
}

2. 隔离存储
#[link(wasm_import_module = "host")]
extern {
    fn storage_get(key_ptr: u32, key_len: u32, ret_ptr: u32, ret_len_ptr: u32) -> i32;
    fn storage_set(key_ptr: u32, key_len: u32, val_ptr: u32, val_len: u32) -> i32;
}

3. HTTP GET 请求
#[link(wasm_import_module = "host")]
extern {
    fn http_get(url_ptr: u32, url_len: u32,
                headers_ptr: u32, headers_len: u32,
                ret_ptr: u32, ret_len_ptr: u32) -> i32;
}

// 调用示例（需预先在内存中放置 URL 字符串）
let url = "https://httpbin.org/get";
let (ptr, len) = (url.as_ptr() as u32, url.len() as u32);
let mut buf = [0u8; 1024];
let ret_ptr = buf.as_mut_ptr() as u32;
let mut ret_len = 0u32;
unsafe {
    http_get(ptr, len, 0, 0, ret_ptr, &mut ret_len as *mut u32);
}
let response = String::from_utf8_lossy(&buf[..ret_len as usize]);

文件写入
#[link(wasm_import_module = "host")]
extern {
    fn workspace_write(path_ptr: u32, path_len: u32,
                       content_ptr: u32, content_len: u32) -> i32;
}

let path = "test.txt";
let content = b"Hello, WASM!";
unsafe {
    workspace_write(path.as_ptr() as u32, path.len() as u32,
                    content.as_ptr() as u32, content.len() as u32);
}

5. 目录列表
#[link(wasm_import_module = "host")]
extern {
    fn workspace_list(path_ptr: u32, path_len: u32,
                      ret_ptr: u32, ret_len_ptr: u32) -> i32;
}

let path = "";
let mut buf = [0u8; 1024];
let ret_len = 0u32;
unsafe {
    workspace_list(path.as_ptr() as u32, path.len() as u32,
                   buf.as_mut_ptr() as u32, &mut ret_len as *mut u32);
}
let json = String::from_utf8_lossy(&buf[..ret_len as usize]);
// json 为 ["file1.txt","file2.txt","subdir"] 等

环境变量
#[link(wasm_import_module = "host")]
extern {
    fn env_get(key_ptr: u32, key_len: u32, ret_ptr: u32, ret_len_ptr: u32) -> i32;
}

let key = "HOME";
let mut buf = [0u8; 256];
let mut ret_len = 0u32;
unsafe {
    env_get(key.as_ptr() as u32, key.len() as u32,
            buf.as_mut_ptr() as u32, &mut ret_len as *mut u32);
}
let value = String::from_utf8_lossy(&buf[..ret_len as usize]);

7. 随机数
#[link(wasm_import_module = "host")]
extern {
    fn random_bytes(len: u32, ret_ptr: u32, ret_len_ptr: u32) -> i32;
}

let mut bytes = [0u8; 32];
unsafe {
    random_bytes(32, bytes.as_mut_ptr() as u32, std::ptr::null_mut());
}

8. 延迟

#[link(wasm_import_module = "host")]
extern {
    fn sleep_ms(ms: u32) -> i32;
}

unsafe { sleep_ms(1000); } // 休眠 1 秒

注意：所有宿主函数遵循能力基安全模型，调用前需确保插件配置中已授予相应权限（如  http_allowlist 、 env_allowlist  等）