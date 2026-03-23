//! 文件系统相关宿主函数
//!
//! 提供工作区内的文件读写、删除、创建目录、重命名等功能

use wasmtime::{Linker, Caller};
use crate::extension::wasm::{HostState, read_string_from_memory, write_bytes_to_memory};
use anyhow::Result;
use std::fs;

/// 注册所有文件系统相关函数
pub fn register_fs(linker: &mut Linker<HostState>) -> Result<()> {
    // workspace_read: 读取工作区文件
    linker.func_wrap("env", "workspace_read", |mut caller: Caller<'_, HostState>,
                     path_ptr: u32, path_len: u32,
                     result_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
            Some(mem) => mem,
            None => return Err(wasmtime::Error::msg("no memory")),
        };

        // 读取路径
        let path_str = match read_string_from_memory(&mem, &caller, path_ptr, path_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid path")),
        };

        let host_state = caller.data();
        let workspace_root = match &host_state.workspace_root {
            Some(root) => root,
            None => return Err(wasmtime::Error::msg("workspace not configured")),
        };

        // 防止路径遍历
        if path_str.starts_with('/') || path_str.starts_with("..") || path_str.contains("../") {
            eprintln!("Invalid path: {}", path_str);
            return Err(wasmtime::Error::msg("path not allowed"));
        }

        let full_path = workspace_root.join(&path_str);

        // 确保路径在工作区内
        if !full_path.starts_with(workspace_root) {
            eprintln!("Path escapes workspace: {:?}", full_path);
            return Err(wasmtime::Error::msg("path outside workspace"));
        }

        // 读取文件
        let content = match fs::read(&full_path) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to read file: {}", e);
                return Err(wasmtime::Error::msg("read failed"));
            }
        };

        // 限制文件大小
        if content.len() > host_state.max_body_size {
            eprintln!("File too large: {} > {}", content.len(), host_state.max_body_size);
            return Err(wasmtime::Error::msg("file too large"));
        }

        // 写入结果
        write_bytes_to_memory(&mut caller, result_ptr, &content)
            .map_err(|e| wasmtime::Error::msg(e.to_string()))?;

        Ok(0)
    })?;

    // workspace_delete: 删除工作区文件（需确认）
    linker.func_wrap("env", "workspace_delete", |mut caller: Caller<'_, HostState>,
                     path_ptr: u32, path_len: u32,
                     confirm_ptr: u32, confirm_len: u32,
                     result_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
            Some(mem) => mem,
            None => return Err(wasmtime::Error::msg("no memory")),
        };

        // 读取路径
        let path_str = match read_string_from_memory(&mem, &caller, path_ptr, path_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid path")),
        };

        // 读取确认字符串
        let confirm = match read_string_from_memory(&mem, &caller, confirm_ptr, confirm_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid confirm string")),
        };

        // 确认必须是 "yes"
        if confirm != "yes" {
            eprintln!("Delete not confirmed (got: {})", confirm);
            return Err(wasmtime::Error::msg("deletion requires 'yes' confirmation"));
        }

        let host_state = caller.data();
        let workspace_root = match &host_state.workspace_root {
            Some(root) => root,
            None => return Err(wasmtime::Error::msg("workspace not configured")),
        };

        // 防止路径遍历
        if path_str.starts_with('/') || path_str.starts_with("..") || path_str.contains("../") {
            eprintln!("Invalid path: {}", path_str);
            return Err(wasmtime::Error::msg("path not allowed"));
        }

        let full_path = workspace_root.join(&path_str);

        // 确保路径在工作区内
        if !full_path.starts_with(workspace_root) {
            eprintln!("Path escapes workspace: {:?}", full_path);
            return Err(wasmtime::Error::msg("path outside workspace"));
        }

        // 删除文件
        match fs::remove_file(&full_path) {
            Ok(_) => {
                let ok_msg = b"OK";
                write_bytes_to_memory(&mut caller, result_ptr, ok_msg)
                    .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
                Ok(0)
            }
            Err(e) => {
                eprintln!("Failed to delete file: {}", e);
                Err(wasmtime::Error::msg("delete failed"))
            }
        }
    })?;

    // workspace_mkdir: 创建工作区目录（递归创建父目录）
    linker.func_wrap("env", "workspace_mkdir", |mut caller: Caller<'_, HostState>,
                     path_ptr: u32, path_len: u32,
                     result_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
            Some(mem) => mem,
            None => return Err(wasmtime::Error::msg("no memory")),
        };

        let path_str = match read_string_from_memory(&mem, &caller, path_ptr, path_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid path")),
        };

        let host_state = caller.data();
        let workspace_root = match &host_state.workspace_root {
            Some(root) => root,
            None => return Err(wasmtime::Error::msg("workspace not configured")),
        };

        // 防止路径遍历
        if path_str.starts_with('/') || path_str.starts_with("..") || path_str.contains("../") {
            eprintln!("Invalid path: {}", path_str);
            return Err(wasmtime::Error::msg("path not allowed"));
        }

        let full_path = workspace_root.join(&path_str);

        // 确保路径在工作区内（先检查父目录，因为目录可能不存在，所以不能 canonicalize）
        // 我们使用 starts_with 来检查父目录（或即将创建的目录）
        let parent = full_path.parent().unwrap_or(workspace_root);
        if !parent.starts_with(workspace_root) {
            eprintln!("Path escapes workspace: {:?}", full_path);
            return Err(wasmtime::Error::msg("path outside workspace"));
        }

        // 创建目录（递归）
        match fs::create_dir_all(&full_path) {
            Ok(_) => {
                let ok_msg = b"OK";
                write_bytes_to_memory(&mut caller, result_ptr, ok_msg)
                    .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
                Ok(0)
            }
            Err(e) => {
                eprintln!("Failed to create directory: {}", e);
                Err(wasmtime::Error::msg("mkdir failed"))
            }
        }
    })?;

    // workspace_rename: 重命名或移动工作区内的文件/目录
    linker.func_wrap("env", "workspace_rename", |mut caller: Caller<'_, HostState>,
                     src_ptr: u32, src_len: u32,
                     dst_ptr: u32, dst_len: u32,
                     result_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
            Some(mem) => mem,
            None => return Err(wasmtime::Error::msg("no memory")),
        };

        let src = match read_string_from_memory(&mem, &caller, src_ptr, src_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid source path")),
        };
        let dst = match read_string_from_memory(&mem, &caller, dst_ptr, dst_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid dest path")),
        };

        let host_state = caller.data();
        let workspace_root = match &host_state.workspace_root {
            Some(root) => root,
            None => return Err(wasmtime::Error::msg("workspace not configured")),
        };

        // 防止路径遍历
        if src.starts_with('/') || src.starts_with("..") || src.contains("../") {
            eprintln!("Invalid source path: {}", src);
            return Err(wasmtime::Error::msg("source path not allowed"));
        }
        if dst.starts_with('/') || dst.starts_with("..") || dst.contains("../") {
            eprintln!("Invalid dest path: {}", dst);
            return Err(wasmtime::Error::msg("dest path not allowed"));
        }

        let src_full = workspace_root.join(&src);
        let dst_full = workspace_root.join(&dst);

        // 确保源和目标都在工作区内
        if !src_full.starts_with(workspace_root) {
            eprintln!("Source escapes workspace: {:?}", src_full);
            return Err(wasmtime::Error::msg("source outside workspace"));
        }
        if !dst_full.starts_with(workspace_root) {
            eprintln!("Dest escapes workspace: {:?}", dst_full);
            return Err(wasmtime::Error::msg("dest outside workspace"));
        }

        // 执行重命名
        match fs::rename(&src_full, &dst_full) {
            Ok(_) => {
                let ok_msg = b"OK";
                write_bytes_to_memory(&mut caller, result_ptr, ok_msg)
                    .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
                Ok(0)
            }
            Err(e) => {
                eprintln!("Failed to rename: {}", e);
                Err(wasmtime::Error::msg("rename failed"))
            }
        }
    })?;

    Ok(())
}