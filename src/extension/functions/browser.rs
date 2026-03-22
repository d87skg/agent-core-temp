//! 浏览器自动化相关宿主函数
//!
//! 提供打开网页、点击、填写表单、截图等功能，基于 chromiumoxide 控制无头 Chrome。

use wasmtime::{Linker, Caller};
use crate::extension::wasm::{HostState, write_bytes_to_memory, read_string_from_memory};
use anyhow::Result;
use std::sync::{Arc, Mutex};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use futures::StreamExt;
use url::Url;

// 全局浏览器实例（使用 Arc 包裹以实现多线程共享）
lazy_static::lazy_static! {
    static ref BROWSER_INSTANCE: Arc<Mutex<Option<Arc<Browser>>>> = Arc::new(Mutex::new(None));
}

/// 获取或初始化浏览器实例
async fn get_or_init_browser() -> Result<Arc<Browser>, wasmtime::Error> {
    let mut guard = BROWSER_INSTANCE.lock().unwrap();
    if let Some(ref browser) = *guard {
        return Ok(browser.clone()); // clone Arc
    }

    // 配置浏览器（无沙箱模式适合容器环境）
    let config = BrowserConfig::builder()
        .no_sandbox()
        .window_size(1280, 720)
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
    // browser_open: 打开 URL，返回页面 ID（简化：返回 0 表示成功）
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
            browser.new_page(&url_str)
                .await
                .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
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

    // browser_click: 点击指定选择器（CSS 选择器）
    linker.func_wrap("env", "browser_click", |mut caller: Caller<'_, HostState>,
                     selector_ptr: u32, selector_len: u32,
                     result_ptr: u32| -> Result<i32, wasmtime::Error> {
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
            Some(mem) => mem,
            None => return Err(wasmtime::Error::msg("no memory")),
        };
        let selector = match read_string_from_memory(&mem, &caller, selector_ptr, selector_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid selector")),
        };

        let result = tokio::runtime::Handle::current().block_on(async {
            let browser = get_or_init_browser().await?;
            let pages = browser.pages().await.map_err(|e| wasmtime::Error::msg(e.to_string()))?;
            let page = pages.first().ok_or_else(|| wasmtime::Error::msg("no open page"))?;
            page.find_element(&selector).await
                .map_err(|e| wasmtime::Error::msg(e.to_string()))?
                .click().await
                .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
            Ok::<_, wasmtime::Error>(())
        });
        match result {
            Ok(_) => {
                let ok_msg = "OK";
                write_bytes_to_memory(&mut caller, result_ptr, ok_msg.as_bytes())
                    .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
                Ok(0)
            }
            Err(e) => Err(e),
        }
    })?;

    // browser_fill: 在输入框中填写文本
    linker.func_wrap("env", "browser_fill", |mut caller: Caller<'_, HostState>,
                     selector_ptr: u32, selector_len: u32,
                     text_ptr: u32, text_len: u32,
                     result_ptr: u32| -> Result<i32, wasmtime::Error> {
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
            Some(mem) => mem,
            None => return Err(wasmtime::Error::msg("no memory")),
        };
        let selector = match read_string_from_memory(&mem, &caller, selector_ptr, selector_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid selector")),
        };
        let text = match read_string_from_memory(&mem, &caller, text_ptr, text_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid text")),
        };

        let result = tokio::runtime::Handle::current().block_on(async {
            let browser = get_or_init_browser().await?;
            let pages = browser.pages().await.map_err(|e| wasmtime::Error::msg(e.to_string()))?;
            let page = pages.first().ok_or_else(|| wasmtime::Error::msg("no open page"))?;
            page.find_element(&selector).await
                .map_err(|e| wasmtime::Error::msg(e.to_string()))?
                .type_str(&text).await
                .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
            Ok::<_, wasmtime::Error>(())
        });
        match result {
            Ok(_) => {
                let ok_msg = "OK";
                write_bytes_to_memory(&mut caller, result_ptr, ok_msg.as_bytes())
                    .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
                Ok(0)
            }
            Err(e) => Err(e),
        }
    })?;

    // browser_screenshot: 截图，返回 PNG 字节数据
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
            let data = page.screenshot(params).await
                .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
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

    // browser_close: 关闭浏览器
    linker.func_wrap("env", "browser_close", |mut caller: Caller<'_, HostState>,
                     result_ptr: u32| -> Result<i32, wasmtime::Error> {
        // 1. 从全局变量中取出浏览器实例
        let browser_to_close = {
            let mut guard = BROWSER_INSTANCE.lock().unwrap();
            guard.take() // 将全局变量设为 None
        };

        // 2. 在锁外执行异步关闭，避免阻塞
        if let Some(browser_arc) = browser_to_close {
            // 尝试获取唯一所有权
            if let Ok(mut browser) = Arc::try_unwrap(browser_arc) {
                tokio::runtime::Handle::current().block_on(async {
                    if let Err(e) = browser.close().await {
                        eprintln!("Browser close error: {}", e);
                    }
                });
            } else {
                // 如果还有其他引用，则不能直接关闭，但通常不会发生
                eprintln!("Browser has multiple references, cannot close");
            }
        }

        let ok_msg = "OK";
        write_bytes_to_memory(&mut caller, result_ptr, ok_msg.as_bytes())
            .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
        Ok(0)
    })?;

    Ok(())
}