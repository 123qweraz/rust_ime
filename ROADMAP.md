# Rust-IME 架构演进路线图 (Architecture Roadmap)

本项目目前已完成从 0 到 1 的原型开发，实现了基于 TSF (Windows) 和 evdev (Linux) 的输入法核心逻辑。为了将项目推向工业级水平，未来的工作重点将从“功能堆砌”转向“架构治理”。

---

## 📅 阶段一：基础加固与观测 (Foundation & Stability)
*目标：提升系统透明度，规范底层通信，减少低级同步 Bug。*

- [x] **UI 抽象层实现**：已引入 `CandidateDisplay` trait，解耦 Slint 窗口与 Linux 桌面通知。
- [ ] **结构化日志系统**：引入 `tracing` 框架替代 `println!`，支持日志持久化。
- [ ] **IPC 协议规范化**：将跨线程/跨进程通信改为强类型的 `IpcMessage` 枚举。
- [ ] **健壮的错误处理**：减少 `unwrap()`，建立统一的 `AppError` 及 Panic 恢复机制。

---

## 📅 阶段二：状态管理与并发重构 (State & Concurrency)
*目标：解决多线程状态同步冲突，提升 UI 响应速度。*

- [ ] **单一数据源 (SSoT) 架构**：建立全局唯一的 `AppState` 状态机，UI 改为“观察者模式”。
- [ ] **解耦主循环**：将 `main.rs` 职责拆分为独立的 Service（Ipc, Gui, Config, Tray）。
- [ ] **无锁输入流水线**：将 `Processor` 放入独立线程，移除频繁的 `Mutex` 锁定，通过消息驱动提高流畅度。

---

## 📅 阶段三：核心引擎流水线化 (Pipeline Architecture)
*目标：仿照 Rime 架构，将 God Object `Processor` 拆解为可插拔的流水线。*

- [ ] **三段式处理流程**：
    - **Preprocessor**: 处理按键映射、双拼转换、特殊快捷键。
    - **Translator**: 输入解析序列，输出候选词列表。支持 `TableTranslator` (本地), `LuaTranslator` (脚本), `CloudTranslator` (网络)。
    - **Filter**: 结果二次加工（去重、繁简转换、Emoji 过滤）。
- [ ] **Schema 驱动**：通过配置文件定义输入方案，而非在 Rust 代码中硬编码逻辑。

---

## 📅 阶段四：数据层性能优化 (Storage & Speed)
*目标：极速启动，支持百万级超大规模词库。*

- [ ] **静态词库 mmap 化**：使用 Memory Mapped File (如 `fst` 或自定义二进制格式) 加载系统词库，实现零延迟启动。
- [ ] **用户数据持久化**：引入 **SQLite** (或 `sled`) 存储用户词频和学习记录，确保数据一致性与事务安全。
- [ ] **冷热分离**：高频词保留在内存高速 Trie，低频词/长词按需从磁盘索引。

---

## 📅 阶段五：Linux 输入层进化 (Linux Input Evolution)
*目标：从“模拟按键”转向“标准输入协议适配”。*

- [ ] **InputHost 适配器化**：支持用户在设置中切换不同的后端：
    - `HardwareInterceptor` (现有基于 evdev/uinput 方案)。
    - `Fcitx5Frontend` (实现 Fcitx5 D-Bus 协议，支持原生 Wayland 与光标跟随)。
    - [ ] **WaylandProtocol**：直接实现 `text-input-v3` 和 `input-method-v2` 协议，解决免 Root 权限运行和像素级光标跟随问题（优先适配 KDE Plasma 6/KWin）。

    ---

    ## 📅 阶段八：移动端与多端同步 (Mobile & Sync)
    *目标：将 Rust 核心带入移动领域。*

    - [ ] **Android 核心移植**：使用 `uniffi-rs` 或 `jni-rs` 将核心引擎封装为安卓动态库（.so）。
    - [ ] **软键盘 UI 开发**：基于安卓原生系统实现高度自定义的键盘视图，适配触摸交互。
    - [ ] **云同步 (可选)**：建立基于 WebDAV 或私有云的用户词库同步机制，打通手机与电脑的输入习惯。

    ---

> **架构师寄语**：好的代码是演化出来的，不是一次性设计出来的。先保持项目能跑，再通过局部的重构让它跑得更好。

---

## 🔧 低风险重构路线图（按 PR 拆分）

> 目标：在**不改变外部行为**的前提下，先做“结构清理”，再做“并发与边界收口”。每个 PR 都应可独立回滚。

### PR-1：抽离启动参数分发（仅搬运，不改逻辑）
**变更范围**
- 新增 `src/app/cli.rs`（或 `src/bootstrap/cli.rs`），把 `--bench` / `--test` / `--compile-only` / `--register` 等分支从 `main.rs` 抽离。
- `main.rs` 保留一层薄入口：解析参数 -> 调用 `cli::run(args)`。

**风险控制**
- 仅函数搬运 + 单元测试补齐，不改参数语义。
- 用快照测试固定 `--help`（若无 `--help`，固定关键分支输出文本）。

**验收标准**
- `cargo check` 通过。
- 现有命令行开关行为与重构前一致。

---

### PR-2：收口平台分支，建立 `PlatformBootstrap`
**变更范围**
- 新增 `src/platform/bootstrap.rs`，封装 Linux/Windows 的 host 选择与启动。
- `main.rs` 中只保留：`platform::bootstrap::run(...)`。

**风险控制**
- 不改任一 host 的运行逻辑，仅移动分支。
- 保留原日志文本，方便回归对比。

**验收标准**
- Linux：`evdev/ibus/wayland` 的选择逻辑不变。
- Windows：TSF 启动路径不变。

---

### PR-3：引入 `AppContext`，减少参数散弹传递
**变更范围**
- 新增 `AppContext`（持有 `config / processor / tray_tx / gui_tx / app_state`）。
- 将线程启动函数改为接受 `Arc<AppContext>`，减少 5~8 个参数长链传递。

**风险控制**
- 只做“参数形态重构”，禁止改业务分支。
- 每次线程回调入口先加结构化日志（thread 名 + event）。

**验收标准**
- 编译通过，托盘事件（开关输入法、切方案、重载配置）行为不变。

---

### PR-4：配置字符串字段类型化（第一批）
**变更范围**
- 将高风险字符串字段改为 `enum`：
  - `paste_method`
  - `candidate_layout`
  - `theme_mode`
- 保持 serde 兼容：为旧字符串提供反序列化兜底（未知值回退默认值并打印告警）。

**风险控制**
- 只改配置模型与映射，不触碰引擎逻辑。
- 增加反序列化兼容测试（新旧配置样本）。

**验收标准**
- 历史配置文件可无损加载。
- Web 配置保存后字段值稳定（无意外改写）。

---

### PR-5：WebServer 状态瘦身与职责收口
**变更范围**
- 清理 `WebServer` 中未实际使用的状态（如无必要的 `tries`）。
- 将“配置读写 + 通知重载”封装为 `ConfigService`。

**风险控制**
- API 路由与返回结构保持不变。
- 先加回归脚本（最小 smoke）：`/api/config` 读写、`/api/dicts` 列表。

**验收标准**
- Web 配置中心核心接口兼容。
- 启停 WebServer 不影响主输入流程。

---

### PR-6：统一并发入口（线程模型先规范，暂不全面 async 化）
**变更范围**
- 明确规则：
  - 主流程使用 `std::thread` + channel；
  - Web 子系统独立 runtime，但创建与销毁位置固定在单一模块。
- 增加 `shutdown` 信号，避免“后台线程悬挂”。

**风险控制**
- 不强行把所有模块改 Tokio，先规范边界。
- 对共享锁热点打点（锁等待时长日志）。

**验收标准**
- 退出流程可控，无僵尸线程。
- 压测下无明显新增卡顿。

---

### PR-7：去重跨 crate 常量与平台注册逻辑
**变更范围**
- 将 `IME_ID / LANG_PROFILE_ID` 统一到单一来源模块。
- `lib.rs` 与 `main.rs` 通过同一模块复用，避免漂移。

**风险控制**
- GUID 值不变，注册命令不变。
- 增加 Windows 端静态断言（编译期常量一致性）。

**验收标准**
- TSF 注册/反注册行为与重构前一致。

---

### PR-8：补齐“架构防回退”文档与检查
**变更范围**
- 新增 `docs/architecture-boundaries.md`：模块边界、禁止跨层调用清单。
- 在 CI 增加基础检查：禁止 `main.rs` 超过约定复杂度阈值（可先软告警）。

**风险控制**
- 文档先行，不阻断业务提交流程。

**验收标准**
- 新同学可按文档快速定位模块职责。

---

## ✅ 执行策略建议
- 每个 PR 控制在 **200~400 行有效变更**。
- 每个 PR 都要求：
  1. 功能行为不变说明；
  2. 回滚方式；
  3. 最小验证命令清单。
- 合并节奏：先 1~3（结构），再 4~5（模型），最后 6~8（并发与治理）。
