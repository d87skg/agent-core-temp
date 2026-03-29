//! 数据库查询相关宿主函数
//!
//! 提供 SQLite 查询接口，通过白名单限制连接

use wasmtime::{Linker, Caller};
use crate::extension::wasm::{HostState, read_string_from_memory, write_bytes_to_memory};
use anyhow::Result;
use rusqlite::Connection;
use serde_json::json;
use std::sync::Mutex;
use once_cell::sync::Lazy;

// 全局 SQLite 连接缓存（按路径）
static DB_CONNECTIONS: Lazy<Mutex<std::collections::HashMap<String, Connection>>> =
    Lazy::new(|| Mutex::new(std::collections::HashMap::new()));

/// 注册数据库相关函数
pub fn register_database(linker: &mut Linker<HostState>) -> Result<()> {
    // ===================== database_query =====================
    // 输入: JSON {"connection_string": "...", "sql": "...", "params": [...]}
    // 输出: JSON {"rows": [...]}
    linker.func_wrap("env", "database_query",
        |mut caller: Caller<'_, HostState>,
         input_ptr: u32,
         input_len: u32,
         output_ptr: u32|
         -> Result<i32, wasmtime::Error> {

            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(mem) => mem,
                None => return Err(wasmtime::Error::msg("no memory")),
            };
            let input_str = match read_string_from_memory(&mem, &caller, input_ptr, input_len) {
                Ok(s) => s,
                Err(_) => return Err(wasmtime::Error::msg("invalid input")),
            };
            let params_val: serde_json::Value = serde_json::from_str(&input_str)
                .map_err(|e| wasmtime::Error::msg(format!("invalid JSON: {}", e)))?;

            let conn_str = params_val["connection_string"].as_str()
                .ok_or_else(|| wasmtime::Error::msg("missing connection_string"))?;
            let sql = params_val["sql"].as_str()
                .ok_or_else(|| wasmtime::Error::msg("missing sql"))?;
            
            // 处理 query_params：使用 let binding 避免临时值
            let empty_params = vec![];
            let query_params = params_val.get("params")
                .and_then(|p| p.as_array())
                .unwrap_or(&empty_params);

            // 安全检查：连接字符串白名单（示例：只允许 SQLite 文件，限制路径）
            if !conn_str.starts_with("sqlite://") && !conn_str.starts_with("file:") {
                return Err(wasmtime::Error::msg("unsupported connection type"));
            }
            // 提取文件路径（简单处理）
            let path = conn_str.trim_start_matches("sqlite://").trim_start_matches("file:");
            if path.contains("..") || path.starts_with('/') {
                return Err(wasmtime::Error::msg("invalid database path"));
            }

            // 获取或创建连接
            let mut conn_guard = DB_CONNECTIONS.lock().unwrap();
            let conn = conn_guard.entry(conn_str.to_string()).or_insert_with(|| {
                Connection::open(path).expect("failed to open database")
            });

            // 准备 SQL 并获取列名
            let mut stmt = conn.prepare(sql)
                .map_err(|e| wasmtime::Error::msg(format!("prepare failed: {}", e)))?;
            // 明确告诉它是 Vec<String>
let column_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();

            // 转换参数：注意这里需要将每个值转换为字符串，并确保生命周期有效
            // 使用 String 而非 &str 以避免临时借用
            let param_strings: Vec<String> = query_params.iter().map(|v| {
                if v.is_string() { v.as_str().unwrap().to_string() }
                else if v.is_number() { v.to_string() }
                else { String::new() }
            }).collect();
            let param_refs: Vec<&dyn rusqlite::ToSql> = param_strings.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

            // 执行查询
            let mut rows = stmt.query(param_refs.as_slice())
                .map_err(|e| wasmtime::Error::msg(format!("query failed: {}", e)))?;

            let mut result_rows = Vec::new();
            while let Some(row) = rows.next()
                .map_err(|e| wasmtime::Error::msg(format!("row error: {}", e)))? {
                let mut row_map = serde_json::Map::new();
                for (i, col_name) in column_names.iter().enumerate() {
                    let value: String = row.get(i).unwrap_or_default();
                    row_map.insert(col_name.clone(), serde_json::Value::String(value));
                }
                result_rows.push(serde_json::Value::Object(row_map));
            }

            let result = json!({ "rows": result_rows }).to_string();
            let bytes = result.into_bytes();

            write_bytes_to_memory(&mut caller, output_ptr, &bytes)
                .map_err(|e| wasmtime::Error::msg(e.to_string()))?;

            Ok(0)
        })?;

    Ok(())
}