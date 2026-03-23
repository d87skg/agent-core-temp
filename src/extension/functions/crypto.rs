//! 加密相关宿主函数
//!
//! 提供 Ed25519 签名与验证功能

use wasmtime::{Linker, Caller};
use crate::extension::wasm::{HostState, read_string_from_memory, write_bytes_to_memory};
use anyhow::Result;
use ring::signature::{Ed25519KeyPair, UnparsedPublicKey, ED25519};
use serde_json::json;
use data_encoding::HEXLOWER;

/// 注册所有加密相关函数
pub fn register_crypto(linker: &mut Linker<HostState>) -> Result<()> {
    // ===================== crypto_sign_ed25519 =====================
    // 输入: JSON {"private_key_hex": "...", "message": "..."}
    // 输出: JSON {"signature_hex": "..."}
    linker.func_wrap("env", "crypto_sign_ed25519",
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
            let params: serde_json::Value = serde_json::from_str(&input_str)
                .map_err(|e| wasmtime::Error::msg(format!("invalid JSON: {}", e)))?;

            let private_key_hex = params["private_key_hex"].as_str()
                .ok_or_else(|| wasmtime::Error::msg("missing private_key_hex"))?;
            let message = params["message"].as_str()
                .ok_or_else(|| wasmtime::Error::msg("missing message"))?;

            let private_key_bytes = HEXLOWER.decode(private_key_hex.as_bytes())
                .map_err(|e| wasmtime::Error::msg(format!("invalid hex: {}", e)))?;
            let key_pair = Ed25519KeyPair::from_pkcs8(&private_key_bytes)
                .map_err(|e| wasmtime::Error::msg(format!("invalid private key: {}", e)))?;

            let signature = key_pair.sign(message.as_bytes());
            let signature_hex = HEXLOWER.encode(signature.as_ref());

            let result = json!({ "signature_hex": signature_hex }).to_string();
            let bytes = result.into_bytes();

            write_bytes_to_memory(&mut caller, output_ptr, &bytes)
                .map_err(|e| wasmtime::Error::msg(e.to_string()))?;

            Ok(0)
        })?;

    // ===================== crypto_verify_ed25519 =====================
    // 输入: JSON {"public_key_hex": "...", "message": "...", "signature_hex": "..."}
    // 输出: JSON {"valid": true/false}
    linker.func_wrap("env", "crypto_verify_ed25519",
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
            let params: serde_json::Value = serde_json::from_str(&input_str)
                .map_err(|e| wasmtime::Error::msg(format!("invalid JSON: {}", e)))?;

            let public_key_hex = params["public_key_hex"].as_str()
                .ok_or_else(|| wasmtime::Error::msg("missing public_key_hex"))?;
            let message = params["message"].as_str()
                .ok_or_else(|| wasmtime::Error::msg("missing message"))?;
            let signature_hex = params["signature_hex"].as_str()
                .ok_or_else(|| wasmtime::Error::msg("missing signature_hex"))?;

            let public_key_bytes = HEXLOWER.decode(public_key_hex.as_bytes())
                .map_err(|e| wasmtime::Error::msg(format!("invalid public key hex: {}", e)))?;
            let signature_bytes = HEXLOWER.decode(signature_hex.as_bytes())
                .map_err(|e| wasmtime::Error::msg(format!("invalid signature hex: {}", e)))?;

            let public_key = UnparsedPublicKey::new(&ED25519, public_key_bytes);
            let valid = public_key.verify(message.as_bytes(), &signature_bytes).is_ok();

            let result = json!({ "valid": valid }).to_string();
            let bytes = result.into_bytes();

            write_bytes_to_memory(&mut caller, output_ptr, &bytes)
                .map_err(|e| wasmtime::Error::msg(e.to_string()))?;

            Ok(0)
        })?;

    Ok(())
}