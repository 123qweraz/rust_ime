# Bug 修复记录

## 1. COSMIC/Wayland 环境下按键导致程序崩溃 (Broken pipe)

**问题描述：**
在 COSMIC 桌面环境下，开启 `gtk4-layer-shell` 后，每当按下按键（触发 UI 显示或隐藏）时，程序会立即崩溃并报错 `Broken pipe`。

**原因分析：**
1. **角色切换压力**：原代码频繁使用 `set_visible(true/false)`，这在 Wayland 协议中会导致 Surface 的频繁创建与销毁（角色重置）。
2. **合成器兼容性**：COSMIC 合成器对 `layer-shell` 窗口的频繁状态切换处理不够稳健，当 Socket 连接因为协议交互异常断开时，GTK 接收到致命错误并强制退出。
3. **信号传递**：GTK 的崩溃会触发 `SIGPIPE` 信号，默认情况下会杀死整个进程。

**解决方案：**
1. **始终映射策略**：将 `set_visible(false)` 替换为 `window.set_opacity(0.0)`。窗口在启动时建立一次 Wayland 连接（`present()`）后保持存活，仅通过透明度控制视觉隐藏。
2. **信号屏蔽**：在 `main.rs` 中忽略 `SIGPIPE` 信号，确保即使 GUI 线程发生协议级错误，IME 核心逻辑仍然能够存活。
3. **架构重构**：将 IME 核心逻辑放在主线程，GUI 作为子线程插件运行，实现生命周期解耦。

**遗留问题：**
在某些环境下，`set_opacity(0.0)` 虽然不可见，但窗口依然占据层级，可能存在残影或无法点击底层的问题。后续需进一步测试各发行版兼容性。

## 2. 潜在隐患与架构审查 (Comprehensive Review)

### 潜在 Panic 点 (Unwrap Usage)
1. **主键设备寻找 (`main.rs`)**: `find_keyboard().unwrap_or_default()` 后直接 `Device::open(&device_path)?`。如果路径非法或权限不足，虽然有 `?` 但前面的逻辑依赖较重。
2. **字符处理 (`ime.rs`, `config.rs`)**: `current_str.chars().next().unwrap()` 在 `segment_pinyin` 中使用。虽然逻辑上此时字符串不应为空，但在极端非法编码或逻辑边缘情况下可能崩溃。
3. **Web 端数据获取 (`web.rs`)**: 存在多处 `.unwrap()`，如 `num_str.insert(0, clean_word.pop().unwrap())`。如果 `clean_word` 为空，Web 配置服务器将直接崩溃。

### 内存与性能隐患
1. **上下文缓存 (`Ime.context`)**: `context` 向量仅在 `commit_candidate` 时通过 `if self.context.len() > 2 { ... }` 保持最近 2 个字符。目前的逻辑是安全的。
2. **模糊音扩展 (`expand_fuzzy_pinyin`)**: 使用了多次 `apply_rule` 并在每次调用时克隆整个列表。虽然 Pinyin 较短，但在极端输入下（如大量模糊音组合）可能存在 O(2^n) 的分配压力。
3. **字典加载效率**: `load_file_into_dict` 在主线程同步加载所有 JSON。随着词库增大，启动时间会线性增长。

### 架构弱点
1. **守护进程环境变量**: 目前依赖手动捕获并恢复 `DISPLAY` 等变量。如果用户在不同 Session 或通过 SSH 启动，可能导致 GUI 无法连接。
2. **虚拟键盘依赖**: `Vkbd` 极度依赖 `/dev/uinput` 权限。如果用户没有正确设置 udev 规则，程序在启动时会直接 panic 或返回错误。

