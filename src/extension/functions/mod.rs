use wasmtime::Linker;
use crate::extension::wasm::HostState;

/// 🚀 彻底同步化：清空所有子模块注册
/// 确保不触发 Wasmtime 的 "cannot use func_wrap_async" 运行时检查
pub fn register_all(_linker: &mut Linker<HostState>) -> anyhow::Result<()> {
    // 所有的 http::register, fs::register, crypto::register 均已移除
    // 这样 Linker 内部就不会包含任何异步函数定义
    
    Ok(())
}