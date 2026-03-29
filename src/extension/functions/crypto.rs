//! --------------------------------------------------
//! Module: src/extension/functions/crypto.rs
//! Description: 基于 ring 库的加密 Host Functions
//! --------------------------------------------------

use wasmtime::{Linker, Caller};
use crate::extension::wasm::{read_string_from_memory, write_bytes_to_memory, HostState};
use anyhow::Result;
use ring::signature::{self, Ed25519KeyPair};

/// 为 WASM Linker 注册加密相关接口
pub fn register_crypto(linker: &mut Linker<HostState>) -> Result<()> {
    
    // [接口 1] host_crypto_sign: 使用宿主私钥签名
    linker.func_wrap("env", "host_crypto_sign", |mut caller: Caller<'_, HostState>, 
                      data_ptr: u32, data_len: u32, 
                      sig_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        let data = read_string_from_memory(&mut caller, data_ptr, data_len)
            .map_err(|e| wasmtime::Error::msg(format!("Crypto Source Read Error: {}", e)))?;

        // 工业级提示：此处的 Seed 应从加密的环境变量或 Vault 中读取
        let seed = [0u8; 32]; 
        let key_pair = Ed25519KeyPair::from_seed_unchecked(&seed)
            .map_err(|_| wasmtime::Error::msg("Internal Crypto Error: Invalid Key Seed"))?;

        let signature = key_pair.sign(data.as_bytes());
        let sig_bytes = signature.as_ref();

        write_bytes_to_memory(&mut caller, sig_ptr, sig_bytes)
            .map_err(|e: anyhow::Error| wasmtime::Error::msg(e.to_string()))?;

        Ok(0)
    })?;

    // [接口 2] host_crypto_verify: 验证 Ed25519 签名
    linker.func_wrap("env", "host_crypto_verify", |mut caller: Caller<'_, HostState>, 
                      pubkey_ptr: u32, pubkey_len: u32,
                      data_ptr: u32, data_len: u32,
                      sig_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        let pubkey_str = read_string_from_memory(&mut caller, pubkey_ptr, pubkey_len)?;
        let data_str = read_string_from_memory(&mut caller, data_ptr, data_len)?;
        
        let mut sig_bytes = vec![0u8; 64];
        let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
        mem.read(&caller, sig_ptr as usize, &mut sig_bytes)
            .map_err(|_| wasmtime::Error::msg("Memory Read Error"))?;

        let peer_public_key = signature::UnparsedPublicKey::new(&signature::ED25519, pubkey_str.as_bytes());
        
        match peer_public_key.verify(data_str.as_bytes(), &sig_bytes) {
            Ok(_) => Ok(0),  // 成功
            Err(_) => Ok(1), // 失败
        }
    })?;

    Ok(())
}