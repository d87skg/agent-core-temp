use serde_json::{Value, json};
use std::env;

/// 真实调用阿里云 DashScope API
pub fn ai_completion(payload: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    // 1. 获取环境变量中的 API Key
    let api_key = env::var("DASHSCOPE_API_KEY")
        .map_err(|_| "环境变量 DASHSCOPE_API_KEY 未设置")?;

    // 2. 解析参数
    let model = payload.get("model").and_then(|v| v.as_str()).unwrap_or("qwen-plus");
    let prompt = payload.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
    let max_tokens = payload.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(1000);

    if prompt.is_empty() {
        return Err("提示词 (prompt) 不能为空".into());
    }

    println!("🤖 正在调用 AI 模型: {}，提示词长度: {}", model, prompt.len());

    // 3. 构建请求体
    let url = "https://dashscope.aliyuncs.com/api/v1/services/aigc/text-generation/generation";
    let body = json!({
        "model": model,
        "input": {
            "messages": [
                { "role": "system", "content": "You are a professional AI assistant." },
                { "role": "user", "content": prompt }
            ]
        },
        "parameters": {
            "result_format": "message",
            "max_tokens": max_tokens,
            "temperature": 0.7
        }
    });

    // 4. 发送同步请求 (使用 blocking 模式)
    let client = reqwest::blocking::Client::new();
    let response = client.post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()?;

    // 5. 处理结果
    if response.status().is_success() {
        let resp_json: Value = response.json()?;
        let content = resp_json["output"]["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("AI 未返回内容")
            .to_string();

        Ok(json!({
            "result": "success",
            "content": content
        }))
    } else {
        let err_text = response.text().unwrap_or_else(|_| "未知 API 错误".into());
        Err(format!("DashScope API 报错: {}", err_text).into())
    }
}