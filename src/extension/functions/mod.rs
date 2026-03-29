// 文件路径: src/extension/functions/mod.rs

pub mod crypto; // 👈 必须添加这一行，告诉 Rust 这里有一个 crypto.rs 文件

use wasmtime::Linker;
use crate::extension::wasm::HostState;
use crate::extension::functions::crypto::register_crypto; 

pub fn register_all(linker: &mut Linker<HostState>) -> anyhow::Result<()> {
    register_crypto(linker)?;
    Ok(())
}