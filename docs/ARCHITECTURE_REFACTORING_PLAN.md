# Rust IME 架构重构计划

> 创建时间: 2026-03-21
> 最后更新: 2026-03-21

## 目录

1. [问题概述](#问题概述)
2. [问题优先级分类](#问题优先级分类)
3. [阶段一：消除重复代码](#阶段一消除重复代码)
4. [阶段二：职责分离](#阶段二职责分离)
5. [阶段三：平台抽象](#阶段三平台抽象)
6. [阶段四：搜索引擎重构](#阶段四搜索引擎重构)
7. [风险评估](#风险评估)
8. [实施建议](#实施建议)

---

## 问题概述

### 当前架构问题

| 问题 | 影响 | 严重程度 |
|------|------|----------|
| 代码重复 | 可维护性差 | 🔴 高 |
| 状态管理混乱 | 线程安全风险 | 🔴 高 |
| Processor God Object | 难以扩展 | 🟠 中 |
| 配置管理重复 | 数据不一致 | 🟠 中 |
| 平台抽象不足 | 跨平台困难 | 🟡 低 |
| Pipeline 架构不统一 | 性能优化困难 | 🟡 低 |

### 当前模块依赖关系

```
main.rs
├── app/
│   ├── cli.rs
│   └── runtime.rs
├── config.rs (763行)
├── engine/
│   ├── mod.rs
│   ├── processor/
│   │   ├── mod.rs (689行 - 过于庞大)
│   │   ├── handlers.rs
│   │   ├── commands.rs
│   │   ├── fsm.rs
│   │   ├── intents.rs
│   │   ├── punctuation.rs
│   │   └── utils.rs
│   ├── config_manager.rs (332行 - 配置重复)
│   ├── pipeline.rs
│   ├── session.rs
│   ├── trie.rs
│   ├── schemes/
│   │   ├── mod.rs
│   │   ├── chinese.rs
│   │   ├── english.rs
│   │   ├── japanese.rs
│   │   └── stroke.rs
│   └── scheme.rs
├── platform/
│   ├── traits.rs
│   ├── fonts.rs
│   ├── linux/ (4个子模块)
│   └── windows/ (1个子模块)
└── ui/
    ├── mod.rs
    ├── tray.rs
    ├── web.rs
    ├── gui_slint.rs
    └── linux_notify.rs
```

---

## 问题优先级分类

### 🔴 高优先级（影响可维护性和正确性）

#### 1. 代码重复问题

| 位置 | 问题 | 解决方案 |
|------|------|----------|
| `main.rs` 和 `lib.rs` | `IME_ID`, `LANG_PROFILE_ID` 重复定义 | 提取到 `src/constants.rs` |
| `main.rs` 第 2-78 行 | Windows Key 枚举重复定义 | 移动到 `src/engine/keys.rs` |
| `Config` 和 `ConfigManager` | 配置数据重复 | 统一使用 `ArcSwap<Config>` |

**待修改文件**：
- 新建 `src/constants.rs`
- 修改 `src/engine/keys.rs`
- 修改 `src/lib.rs`, `src/main.rs`

#### 2. 状态管理混乱

**当前混用**：
```rust
// 问题代码示例
Arc<Mutex<Processor>>      // main.rs
ArcSwap<UserDictData>     // engine
RwLock<Config>            // main.rs  
Mutex<LruCache<...>>      // pipeline
```

**解决方案**：统一为两种模式
```rust
// 读多写少 → ArcSwap<T>
// 读写混合 → RwLock<T>
```

---

### 🟠 中优先级（影响扩展性）

#### 3. Processor God Object

`src/engine/processor/mod.rs` (689行) 承担了太多职责：

| 职责 | 当前位置 | 建议拆分 |
|------|----------|----------|
| 引擎调度 | Processor | 保持 |
| 状态机 | 内嵌 | 新建 `state.rs` |
| 配置管理 | ConfigManager | 独立 `ConfigStore` |
| 会话管理 | Session | 已有 `InputSession` |
| 命令执行 | 内嵌 | 新建 `command_executor.rs` |
| 词库学习 | 内嵌 | 新建 `learning.rs` |

**重构后结构**：
```
engine/
├── processor/
│   ├── mod.rs          # 精简为协调器 (~100行)
│   ├── state.rs        # 状态机逻辑
│   ├── commands.rs     # 命令执行
│   └── context.rs      # 处理器上下文
├── config_store.rs     # 配置统一管理
├── learning.rs         # 学习引擎
```

#### 4. 配置管理重复

`config.rs` (763行) + `ConfigManager` (332行) = ~1100 行重复配置代码

**解决方案**：
```rust
// src/engine/config_store.rs
pub struct ConfigStore {
    config: ArcSwap<Config>,  // 主配置
    // 直接从 Config 派生，非重复字段
}

impl ConfigStore {
    pub fn new() -> Self { ... }
    pub fn apply(&self, config: Config) { ... }
    pub fn get(&self) -> Arc<Config> { self.config.load() }
}
```

---

### 🟡 低优先级（改进代码质量）

#### 5. 平台抽象改进

**当前问题**：
- `#[cfg(target_os = "...")]` 散布在代码中
- `app/runtime.rs` 包含大量平台判断逻辑

**解决方案**：引入 Trait 抽象
```rust
// src/platform/mod.rs
pub trait InputBackend {
    fn new(...) -> Self;
    fn run(&mut self) -> Result<()>;
    fn on_key_event(&mut self, event: InputEvent) -> Action;
}
```

#### 6. 搜索引擎 Pipeline 改进

**当前问题**：
- `SearchEngine` 同时支持 Pipeline 和 Scheme 两套查找逻辑
- 缓存用 `Mutex` 而非 `ArcSwap`

**解决方案**：
```rust
// 统一为 Pipeline 模式
pub struct SearchEngine {
    pipelines: ArcSwap<HashMap<String, Arc<Pipeline>>>,
    cache: ArcSwap<LruCache<SearchCacheKey, Vec<Candidate>>>,
}
```

---

## 阶段一：消除重复代码

> 预计时间: 1-2 天
> 风险等级: 低

### 1.1 创建 src/constants.rs

**新建文件**: `src/constants.rs`

```rust
// Windows IME GUIDs
#[cfg(windows)]
pub const IME_ID: windows::core::GUID = 
    windows::core::GUID::from_u128(0xc03c9525_2c5e_4959_9988_51787281d523);

#[cfg(windows)]
pub const LANG_PROFILE_ID: windows::core::GUID = 
    windows::core::GUID::from_u128(0xc03c9525_2c5e_4959_9988_51787281d524);

// Virtual Key 枚举 (从 main.rs 移动)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
#[repr(u32)]
pub enum Key {
    KEY_A = 0,
    KEY_B,
    // ... 其他按键
    KEY_Z = 25,
    KEY_0 = 26,
    // ...
}
```

**修改文件**:
- `src/lib.rs` - 移除 `IME_ID`, `LANG_PROFILE_ID` 定义，改为引用
- `src/main.rs` - 移除 Key 枚举和 GUID 定义

### 1.2 统一配置管理

**新建文件**: `src/engine/config_store.rs`

```rust
use arc_swap::ArcSwap;
use std::sync::Arc;

pub struct ConfigStore {
    config: ArcSwap<Config>,
    // 仅存储派生状态，不复制 Config 数据
    pub learned_words: Arc<ArcSwap<UserDictData>>,
    pub usage_history: Arc<ArcSwap<UserDictData>>,
    pub ngram_history: Arc<ArcSwap<UserDictData>>,
}

impl ConfigStore {
    pub fn new() -> Self {
        Self {
            config: ArcSwap::from_pointee(Config::default_config()),
            learned_words: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            usage_history: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            ngram_history: Arc::new(ArcSwap::from_pointee(HashMap::new())),
        }
    }

    pub fn get_config(&self) -> Arc<Config> {
        self.config.load()
    }

    pub fn apply_config(&self, new_config: Config) {
        self.config.store(Arc::new(new_config));
    }
}
```

**修改文件**:
- `src/engine/config_manager.rs` - 重构为使用 ConfigStore
- `src/engine/processor/mod.rs` - 更新引用
- `src/main.rs` - 统一使用 ConfigStore

### 1.3 统一状态管理

**修改文件**: `src/main.rs`

```rust
// 修改前
let processor = Arc::new(Mutex::new(processor_obj));

// 修改后
let processor = Arc::new(RwLock::new(processor_obj));
```

---

## 阶段二：职责分离

> 预计时间: 3-5 天
> 风险等级: 中

### 2.1 拆分 Processor

**新建文件**: `src/engine/processor/state.rs`

```rust
use crate::engine::keys::VirtualKey;
use crate::engine::ModifierState;
use crate::engine::processor::{ImeState, FilterMode};

#[derive(Debug, Clone, PartialEq)]
pub enum FsmEffect {
    PassThrough,
    Consume,
    Alert,
    UpdateLookup,
    Commit首选,
    CommitRaw,
    Clear,
}

pub struct StateManager {
    current_state: ImeState,
    // 双击检测状态
    last_tap_key: Option<VirtualKey>,
    last_tap_time: Option<Instant>,
    // 长按检测状态
    key_press_info: Option<(VirtualKey, Instant)>,
    long_press_triggered: bool,
}

impl StateManager {
    pub fn transition(&mut self, input: &FsmInput) -> FsmEffect {
        // 状态转移逻辑
    }

    pub fn reset(&mut self) {
        self.last_tap_key = None;
        self.last_tap_time = None;
        self.key_press_info = None;
        self.long_press_triggered = false;
    }
}
```

**新建文件**: `src/engine/processor/command_executor.rs`

```rust
use crate::engine::processor::{Processor, Action, Command};

pub struct CommandExecutor;

impl CommandExecutor {
    pub fn execute(processor: &mut Processor, cmd: Command) -> Action {
        match cmd {
            Command::NextPage => Self::next_page(processor),
            Command::PrevPage => Self::prev_page(processor),
            // ...
        }
    }

    fn next_page(processor: &mut Processor) -> Action {
        let page_size = processor.config.page_size;
        // ...
    }
}
```

**修改文件**: `src/engine/processor/mod.rs`

```rust
// 简化后的协调器
pub struct Processor {
    pub session: InputSession,
    pub state_manager: StateManager,
    pub config_store: ConfigStore,
    pub engine: SearchEngine,
    // ... 精简后的字段
}

impl Processor {
    pub fn handle_key(&mut self, event: InputEvent) -> Action {
        // 协调各个模块
        let effect = self.state_manager.transition(&input);
        self.execute_effect(effect)
    }

    fn execute_effect(&mut self, effect: FsmEffect) -> Action {
        match effect {
            FsmEffect::UpdateLookup => self.lookup(),
            // ...
        }
    }
}
```

### 2.2 创建学习引擎

**新建文件**: `src/engine/learning.rs`

```rust
use arc_swap::ArcSwap;

pub struct LearningEngine {
    usage_history: Arc<ArcSwap<UserDictData>>,
    ngram_history: Arc<ArcSwap<UserDictData>>,
    learned_words: Arc<ArcSwap<UserDictData>>,
    db: Option<sled::Db>,
}

impl LearningEngine {
    pub fn record_usage(&self, profile: &str, pinyin: &str, word: &str, context: Option<&str>) {
        // 记录调频
        // 记录 N-Gram
        // 记录新词
    }

    pub fn get_frequency_boost(&self, profile: &str, pinyin: &str) -> f64 {
        // 计算调频权重
    }
}
```

---

## 阶段三：平台抽象

> 预计时间: 2-3 天
> 风险等级: 中

### 3.1 定义 InputBackend Trait

**修改文件**: `src/platform/traits.rs`

```rust
use crate::engine::InputEvent;
use crate::engine::processor::Action;
use crate::config::Config;

pub trait InputBackend: Send {
    fn new(...) -> Self;
    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    
    fn on_key_event(&mut self, event: InputEvent) -> Action {
        Action::PassThrough
    }
    
    fn set_config(&mut self, config: Arc<Config>) {
        self.config = config;
    }
}

pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
```

### 3.2 重构平台模块

**修改文件**: `src/platform/linux/mod.rs`

```rust
pub mod evdev_host;
pub mod vkbd;
pub mod wayland;
pub mod ibus_host;

pub use evdev_host::EvdevBackend;
pub use wayland::WaylandBackend;
pub use ibus_host::IBusBackend;

pub fn create_backend(args: &[String], ...) -> Box<dyn InputBackend> {
    match parse_backend(args) {
        BackendType::Wayland => Box::new(WaylandBackend::new(...)),
        BackendType::Evdev => Box::new(EvdevBackend::new(...)),
        BackendType::IBus => Box::new(IBusBackend::new(...)),
    }
}
```

### 3.3 简化 app/runtime.rs

**修改文件**: `src/app/runtime.rs`

```rust
pub fn run_input_host(
    args: &[String],
    processor: Arc<RwLock<Processor>>,
    gui_tx: Sender<GuiEvent>,
    config: Arc<RwLock<Config>>,
    tray_tx: Sender<TrayEvent>,
) -> Result<(), Box<dyn Error>> {
    let backend = platform::create_backend(args, processor.clone(), gui_tx, tray_tx)?;
    backend.run()
}
```

---

## 阶段四：搜索引擎重构

> 预计时间: 2-3 天
> 风险等级: 中

### 4.1 统一 Pipeline 架构

**修改文件**: `src/engine/pipeline.rs`

```rust
#[derive(Clone)]
pub struct SearchEngine {
    trie_paths: HashMap<String, (PathBuf, PathBuf)>,
    syllables: Arc<HashSet<String>>,
    learning_engine: Arc<LearningEngine>,
    pipelines: ArcSwap<HashMap<String, Arc<Pipeline>>>,  // 改为 ArcSwap
    cache: ArcSwap<LruCache<SearchCacheKey, Vec<Candidate>>>,  // 改为 ArcSwap
    schemes: Arc<HashMap<String, Box<dyn InputScheme>>>,
}

impl SearchEngine {
    pub fn search(&self, query: SearchQuery) -> (Vec<Candidate>, Vec<String>) {
        // 统一使用 Pipeline
        if let Some(pipeline) = self.get_or_create_pipeline(query.profile) {
            return pipeline.run(...);
        }
        (vec![], vec![])
    }
}
```

### 4.2 消除 Scheme Fallback

**策略**: 逐步将 Scheme 逻辑迁移到 Pipeline

```rust
// Scheme 实现改为 Pipeline Filter
pub struct ChineseFilter {
    enable_fuzzy: bool,
    enable_abbreviation: bool,
}

impl Filter for ChineseFilter {
    fn filter(&self, input: &str, candidates: Vec<Candidate>, config: &Config) -> Vec<Candidate> {
        // 移自 ChineseScheme 的后处理逻辑
    }
}
```

---

## 风险评估

| 重构项 | 风险 | 影响范围 | 缓解措施 |
|--------|------|----------|----------|
| 配置管理统一 | 中 | 全局 | 先保留双轨，逐步迁移 |
| Processor 拆分 | 高 | 输入处理 | 逐函数迁移，保持功能测试 |
| 平台抽象 | 中 | 跨平台 | 保留 #[cfg] 回退 |
| Pipeline 统一 | 低 | 搜索功能 | 添加集成测试 |

### 风险缓解策略

1. **Git 分支策略**
   ```bash
   git checkout -b refactor/phase1-dedup
   # 完成阶段一后合并
   git checkout -b refactor/phase2-processor
   ```

2. **渐进式迁移**
   - 每个阶段独立可运行
   - 使用 feature flag 控制新/旧代码路径

3. **测试策略**
   - 单元测试覆盖核心逻辑
   - 集成测试覆盖输入输出

---

## 实施建议

### 推荐实施顺序

```
阶段一 (1-2天) → 阶段二 (3-5天) → 阶段三 (2-3天) → 阶段四 (2-3天)
     ↓                ↓                ↓                ↓
   低风险           高风险           中风险           低风险
   高收益           大收益           中收益           中收益
```

### 阶段一详解 (消除重复)

**目标**: 零风险重构，建立统一代码基础

1. 创建 `src/constants.rs`
2. 统一 ConfigStore
3. 统一状态管理

**验收标准**:
- [ ] `cargo build --all-targets` 通过
- [ ] 运行时功能不变
- [ ] 代码重复率下降 (使用 `cargo +nightly install cargo-diet` 检测)

### 阶段二详解 (职责分离)

**目标**: Processor 从 689 行减少到 ~150 行

**验收标准**:
- [ ] Processor 核心逻辑不增加
- [ ] 每个新模块独立可测试
- [ ] 性能不下降

---

## 附录

### A. 相关资源

- [Rust 异步编程指南](https://rust-lang.github.io/async-book/)
- [ArcSwap 文档](https://docs.rs/arc-swap/)
- [状态机模式](https://rust-unofficial.github.io/patterns/patterns/behavioural/state.html)

### B. 代码量统计

| 模块 | 当前行数 | 目标行数 |
|------|----------|----------|
| processor/mod.rs | 689 | ~150 |
| config.rs | 763 | 保留 |
| ConfigManager | 332 | 融入 ConfigStore |
| pipeline.rs | 529 | ~400 |

### C. 里程碑

- [ ] **M1**: 完成阶段一，代码重复问题消除
- [ ] **M2**: 完成阶段二，Processor 职责清晰
- [ ] **M3**: 完成阶段三，平台抽象完善
- [ ] **M4**: 完成阶段四，Pipeline 架构统一

---

*文档版本: 1.0.0*
