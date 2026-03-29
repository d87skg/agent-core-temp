//! 成本统计与配额管理模块
//! 提供 WASM 插件消耗追踪及配额限制

use std::sync::atomic::{AtomicU64, Ordering};
use wasmtime::Linker;
use crate::extension::wasm::HostState;
use anyhow::Result;

pub struct CostTracker {
    total_spent_cents: AtomicU64,
    budget_cents: AtomicU64,
}

impl CostTracker {
    pub fn new(budget: u64) -> Self {
        Self {
            total_spent_cents: AtomicU64::new(0),
            budget_cents: AtomicU64::new(budget),
        }
    }

    /// 记录消耗
    pub fn record_cost(&self, cents: u64) {
        self.total_spent_cents.fetch_add(cents, Ordering::SeqCst);
    }

    /// 检查是否超出预算
    pub fn is_over_budget(&self) -> bool {
        let spent = self.total_spent_cents.load(Ordering::SeqCst);
        let budget = self.budget_cents.load(Ordering::SeqCst);
        spent >= budget
    }

    /// 获取当前总消耗（以分为单位）
    pub fn get_total_spent(&self) -> u64 {
        self.total_spent_cents.load(Ordering::SeqCst)
    }

    /// 设置新的预算
    #[allow(dead_code)]
    pub fn set_budget(&self, budget_cents: u64) {
        self.budget_cents.store(budget_cents, Ordering::SeqCst);
    }
}

// ===================== [WASM 注册逻辑] =====================

/// 注册成本相关宿主函数
/// 解决 mod.rs 中 register_cost 找不到的问题
pub fn register_cost(_linker: &mut Linker<HostState>) -> Result<()> {
    // 预留位置：未来可以在这里注册给 WASM 调用的 cost_check 等函数
    Ok(())
}