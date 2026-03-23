//! WASM宿主函数模块化拆分
//!
//! 按功能分类组织所有宿主函数，保持代码清晰

pub mod http;
pub mod fs;
pub mod security;
pub mod cost;
pub mod browser;
pub mod crypto;
pub mod database;

use wasmtime::Linker;
use crate::extension::wasm::HostState;
use anyhow::Result;

/// 注册所有新增的宿主函数
pub fn register_all(linker: &mut Linker<HostState>) -> Result<()> {
    http::register_http(linker)?;
    fs::register_fs(linker)?;
    security::register_security(linker)?;
    cost::register_cost(linker)?;
    browser::register_browser(linker)?;
    crypto::register_crypto(linker)?;
    database::register_database(linker)?;
    Ok(())
}