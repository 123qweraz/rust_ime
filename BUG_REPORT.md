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
