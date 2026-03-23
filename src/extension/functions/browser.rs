//! 浏览器自动化相关宿主函数
//!
//! 提供打开网页、点击、填写表单、截图等功能，基于 chromiumoxide 控制无头 Chrome。

use wasmtime::{Linker, Caller};
use crate::extension::wasm::{HostState, read_string_from_memory, write_bytes_to_memory};
use anyhow::Result;
use std::sync::{Arc, Mutex};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use futures::StreamExt;
use url::Url;
use lazy_static::lazy_static;

// 全局浏览器实例
lazy_static! {
    static ref BROWSER_INSTANCE: Arc<Mutex<Option<Arc<Browser>>>> = Arc::new(Mutex::new(None));
}

/// 获取或初始化浏览器实例
async fn get_or_init_browser() -> Result<Arc<Browser>, wasmtime::Error> {
    let mut guard = BROWSER_INSTANCE.lock().unwrap();
    if let Some(ref browser) = *guard {
        return Ok(browser.clone());
    }

    // 构建配置，添加多个启动参数以提高兼容性
    let config = BrowserConfig::builder()
        .no_sandbox()
        .window_size(1280, 720)
        .args(vec![
            "--headless".to_string(),
            "--disable-gpu".to_string(),
            "--disable-software-rasterizer".to_string(),
            "--disable-dev-shm-usage".to_string(),
        ])
        .build()
        .map_err(wasmtime::Error::msg)?;

    let (browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| wasmtime::Error::msg(e.to_string()))?;

    // 后台运行事件处理器
    tokio::spawn(async move {
        while let Some(item) = handler.next().await {
            if let Err(e) = item {
                eprintln!("Browser handler error: {}", e);
                break;
            }
        }
    });

    let browser_arc = Arc::new(browser);
    *guard = Some(browser_arc.clone());
    Ok(browser_arc)
}

/// 注册所有浏览器相关函数
pub fn register_browser(linker: &mut Linker<HostState>) -> Result<()> {
    // browser_open: 打开 URL
    linker.func_wrap("env", "browser_open", |mut caller: Caller<'_, HostState>,
                     url_ptr: u32, url_len: u32,
                     result_ptr: u32| -> Result<i32, wasmtime::Error> {
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
            Some(mem) => mem,
            None => return Err(wasmtime::Error::msg("no memory")),
        };
        let url_str = match read_string_from_memory(&mem, &caller, url_ptr, url_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid URL")),
        };

        // 域名白名单检查
        let host_state = caller.data();
        let url = match Url::parse(&url_str) {
            Ok(u) => u,
            Err(e) => {
                eprintln!("Invalid URL: {}", e);
                return Err(wasmtime::Error::msg("invalid URL"));
            }
        };
        let domain = url.host_str().unwrap_or("");
        if !host_state.http_allowlist.iter().any(|allowed| domain == allowed || domain.ends_with(&format!(".{}", allowed))) {
            eprintln!("Domain not allowed: {}", domain);
            return Err(wasmtime::Error::msg("domain not allowed"));
        }

        let result = tokio::runtime::Handle::current().block_on(async {
            let browser = get_or_init_browser().await?;
            browser.new_page(&url_str).await.map_err(|e| wasmtime::Error::msg(e.to_string()))?;
            Ok::<_, wasmtime::Error>(0)
        });
        match result {
            Ok(page_id) => {
                let ok_msg = format!("{}", page_id);
                write_bytes_to_memory(&mut caller, result_ptr, ok_msg.as_bytes())
                    .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
                Ok(0)
            }
            Err(e) => Err(e),
        }
    })?;

    // browser_screenshot: 截图
    linker.func_wrap("env", "browser_screenshot", |mut caller: Caller<'_, HostState>,
                     result_ptr: u32| -> Result<i32, wasmtime::Error> {
        let result = tokio::runtime::Handle::current().block_on(async {
            let browser = get_or_init_browser().await?;
            let pages = browser.pages().await.map_err(|e| wasmtime::Error::msg(e.to_string()))?;
            let page = pages.first().ok_or_else(|| wasmtime::Error::msg("no open page"))?;
            let params = ScreenshotParams::builder()
                .format(CaptureScreenshotFormat::Png)
                .full_page(true)
                .build();
            let data = page.screenshot(params).await.map_err(|e| wasmtime::Error::msg(e.to_string()))?;
            Ok::<_, wasmtime::Error>(data)
        });
        match result {
            Ok(data) => {
                write_bytes_to_memory(&mut caller, result_ptr, &data)
                    .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
                Ok(0)
            }
            Err(e) => Err(e),
        }
    })?;

    // browser_close: 关闭浏览器（释放资源）
    linker.func_wrap("env", "browser_close", |mut caller: Caller<'_, HostState>,
                     result_ptr: u32| -> Result<i32, wasmtime::Error> {
        let mut guard = BROWSER_INSTANCE.lock().unwrap();
        if let Some(browser) = guard.take() {
            drop(browser);
        }
        let ok_msg = "OK";
        write_bytes_to_memory(&mut caller, result_ptr, ok_msg.as_bytes())
            .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
        Ok(0)
    })?;

    Ok(())
}

// ===================== 辅助函数（供模拟插件调用） =====================

/// 打开指定 URL（供模拟插件使用）
pub async fn test_browser_open(url: &str) -> Result<(), wasmtime::Error> {
    let browser = get_or_init_browser().await?;
    browser.new_page(url).await.map_err(|e| wasmtime::Error::msg(e.to_string()))?;
    Ok(())
}

/// 截图并返回 PNG 字节数据（供模拟插件使用）
pub async fn test_browser_screenshot() -> Result<Vec<u8>, wasmtime::Error> {
    let browser = get_or_init_browser().await?;
    let pages = browser.pages().await.map_err(|e| wasmtime::Error::msg(e.to_string()))?;
    let page = pages.first().ok_or_else(|| wasmtime::Error::msg("no open page"))?;
    let params = ScreenshotParams::builder()
        .format(CaptureScreenshotFormat::Png)
        .full_page(true)
        .build();
    let data = page.screenshot(params).await.map_err(|e| wasmtime::Error::msg(e.to_string()))?;
    Ok(data)
}