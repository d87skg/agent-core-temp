@'
# agent-core 5分钟演示脚本

## 0:00-0:30 开场
“大家好，今天展示 Web4 级基础设施 agent-core 的幂等性核心。”

## 0:30-1:30 正常执行
运行任务，Dashboard 显示 Pending→Processing→Completed。

## 1:30-2:30 模拟脑裂
执行 `./simulate_partition.sh`，Dashboard 变红，两个区域进入脑裂状态。

## 2:30-3:30 Fencing Token 展示
观察两个节点 Token 版本，只有最新 Token 能写入，旧节点被拒。

## 3:30-4:30 触发自愈
执行 `./recover_partition.sh`，Dashboard 几秒内变绿，CRDT 进度条拉满，状态自动收敛。

## 4:30-5:00 总结
“数学证明 + 工程实现 = 跨云自愈 Exactly-Once，成本仅为传统方案 1/10。”
'@ | Out-File -FilePath investor-package/04-demo-script.md -Encoding UTF8