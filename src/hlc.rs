// src/hlc.rs
use std::time::{SystemTime, UNIX_EPOCH};

/// 混合逻辑时钟 (Hybrid Logical Clock)
/// 结合了物理时钟和逻辑计数器，确保单调递增且能捕获因果关系
#[derive(Debug, Clone)]
pub struct HlcClock {
    /// 物理时间戳（毫秒）
    wall_time: u64,
    /// 逻辑计数器
    logical: u32,
    /// 测试用的偏移量（毫秒），用于模拟时钟回拨（始终存在，测试和正式编译均可用）
    offset: i64,
}

impl HlcClock {
    /// 创建一个新的 HLC，以当前系统时间初始化
    pub fn new() -> Self {
        let wall_time = Self::current_time_millis();
        Self {
            wall_time,
            logical: 0,
            offset: 0, // 始终初始化为 0
        }
    }

    /// 获取当前物理时间（毫秒）
    fn current_time_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// 生成下一个时间戳
    pub fn now(&mut self) -> u64 {
        let physical = Self::current_time_millis();
        // 应用偏移量（非测试编译时 offset = 0，无影响）
        let physical = if self.offset >= 0 {
            physical.saturating_add(self.offset as u64)
        } else {
            physical.saturating_sub((-self.offset) as u64)
        };

        if physical > self.wall_time {
            // 物理时间前进，重置逻辑计数器
            self.wall_time = physical;
            self.logical = 0;
        } else {
            // 物理时间未变或回拨，增加逻辑计数器
            self.logical += 1;
        }
        // 组合时间戳：高64位为物理时间，低32位为逻辑计数器
        (self.wall_time << 32) | (self.logical as u64)
    }

    /// 接收另一个 HLC 的时间戳，并更新自身（用于事件接收）
    pub fn receive(&mut self, received_ts: u64) {
        let received_wall = received_ts >> 32;
        let received_logical = (received_ts & 0xFFFFFFFF) as u32;
        let physical = Self::current_time_millis();

        self.wall_time = self.wall_time.max(physical).max(received_wall);
        if self.wall_time == physical && self.wall_time == received_wall {
            self.logical = self.logical.max(received_logical) + 1;
        } else if self.wall_time == received_wall {
            self.logical = received_logical + 1;
        } else {
            self.logical = 0;
        }
    }

    /// 手动设置时间偏移（毫秒），用于模拟时钟回拨（测试用）
    pub fn set_manual_offset(&mut self, offset: i64) {
        self.offset = offset;
    }
}

impl Default for HlcClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hlc_monotonic() {
        let mut hlc = HlcClock::new();
        let t1 = hlc.now();
        let t2 = hlc.now();
        assert!(t2 > t1);
    }

    #[test]
    fn test_hlc_receive() {
        let mut hlc1 = HlcClock::new();
        let mut hlc2 = HlcClock::new();
        let ts1 = hlc1.now();
        hlc2.receive(ts1);
        let ts2 = hlc2.now();
        assert!(ts2 > ts1);
    }

    #[test]
    fn test_hlc_rollback() {
        let mut hlc = HlcClock::new();
        let t1 = hlc.now();
        // 模拟时钟回拨 1000 毫秒
        hlc.set_manual_offset(-1000);
        let t2 = hlc.now();
        // 即使物理时间回拨，HLC 也应保证时间戳递增
        assert!(t2 > t1);
    }
}