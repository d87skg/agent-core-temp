// examples/wasm_plugin.rs
use agent_core_temp::extension::load_extension;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 你需要先编译一个 wasm 文件
    let plugin = load_extension("path/to/your/plugin.wasm")?;
    
    println!("Plugin: {} v{}", plugin.name(), plugin.version());
    plugin.init()?;
    
    // 调用插件方法
    let result = plugin.call("greet", serde_json::json!({"name": "World"}))?;
    println!("Result: {}", result);
    
    Ok(())
}