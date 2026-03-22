//! 成本控制相关宿主函数
//!
//! 提供token消耗追踪、预算熔断等功能

use wasmtime::{Linker, Caller};
use crate::extension::wasm::HostState;
use anyhow::Result;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use lazy_static::lazy_static;

// 全局成本追踪器（示例实现，后续可集成到observability模块）
lazy_static! {
    static ref COST_TRACKER: Arc<CostTracker> = Arc::new(CostTracker::new());
}

struct CostTracker {
    total_tokens: AtomicU64,
    total_cost_cents: AtomicU64,  // 以美分为单位
    budget_cents: AtomicU64,
}

impl CostTracker {
    fn new() -> Self {
        Self {
            total_tokens: AtomicU64::new(0),
            total_cost_cents: AtomicU64::new(0),
            budget_cents: AtomicU64::new(1000), // 默认预算 $10
        }
    }

    fn add_tokens(&self, tokens: u64, cost_cents: u64) -> bool {
        let new_total = self.total_tokens.fetch_add(tokens, Ordering::Relaxed) + tokens;
        let new_cost = self.total_cost_cents.fetch_add(cost_cents, Ordering::Relaxed) + cost_cents;
        
        // 检查是否超过预算
        let budget = self.budget_cents.load(Ordering::Relaxed);
        if new_cost > budget {
            eprintln!("Budget exceeded: {} > {}", new_cost, budget);
            false
        } else {
            eprintln!("Cost tracking: tokens={}, total={}, cost={}¢, total={}¢", 
                      tokens, new_total, cost_cents, new_cost);
            true
        }
    }

    fn set_budget(&self, budget_cents: u64) {
        self.budget_cents.store(budget_cents, Ordering::Relaxed);
    }
}

/// 注册所有成本控制相关函数
pub fn register_cost(linker: &mut Linker<HostState>) -> Result<()> {
    // cost_tracker: 记录token消耗
    linker.func_wrap("env", "cost_tracker", |_caller: Caller<'_, HostState>,
                     tokens: u32,
                     cost_cents: u32,
                     _result_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        let within_budget = COST_TRACKER.add_tokens(tokens as u64, cost_cents as u64);
        
        // TODO: 后续集成到 observability 模块
        // crate::observability::record_metric("cost.tokens", tokens as f64);
        // crate::observability::record_metric("cost.cents", cost_cents as f64);
        
        if within_budget {
            Ok(0)
        } else {
            Err(wasmtime::Error::msg("budget exceeded"))
        }
    })?;

    Ok(())
}