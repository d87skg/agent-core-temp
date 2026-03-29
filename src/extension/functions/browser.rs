use anyhow::Result;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::handler::viewport::Viewport;
use futures::StreamExt;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::time::Duration;

pub async fn screenshot(payload: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let url = payload.get("url").and_then(|v| v.as_str()).ok_or("Missing URL")?;
    let save_path = payload.get("save_path").and_then(|v| v.as_str()).unwrap_or("screenshot.png");

    let config = BrowserConfig::builder()
        .request_timeout(Duration::from_secs(30))
        .window_size(1920, 1080)
        .viewport(Viewport {
            width: 1920,
            height: 1080,
            ..Default::default()
        })
        .no_sandbox()
        .arg("--disable-gpu")
        .build()
        .map_err(|e| anyhow::anyhow!(e))?;

    // 修正点：必须是 mut browser 才能执行后面的 .close().await
    let (mut browser, mut handler) = Browser::launch(config).await?;

    let handle = tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            if let Err(_) = h {
                break;
            }
        }
    });

    let page = browser.new_page(url).await?;
    
    // 等待页面加载和渲染
    tokio::time::sleep(Duration::from_secs(3)).await;

    page.save_screenshot(
        chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams::default(),
        PathBuf::from(save_path),
    )
    .await?;

    // 修正点：现在可以安全关闭浏览器了
    browser.close().await?;
    handle.abort();

    Ok(json!({
        "status": "success",
        "save_path": save_path,
        "url": url
    }))
}

pub async fn open(payload: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let url = payload.get("url").and_then(|v| v.as_str()).ok_or("Missing URL")?;
    Ok(json!({ "status": "success", "url": url }))
}

// 为 WASM 导出提供的注册接口（保持现状）
pub fn register_browser(_linker: &mut wasmtime::Linker<crate::extension::wasm::HostState>) -> Result<()> {
    Ok(())
}