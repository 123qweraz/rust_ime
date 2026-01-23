# Rust-IME

一个基于 Rust 开发的 Linux 系统级输入法框架。它通过监听底层键盘事件 (`/dev/input/event`) 并利用虚拟键盘 (uinput) 注入字符，实现了与具体显示协议（Wayland/X11/TTY）无关的全局输入能力。

## 核心特性

- **底层驱动级监听**：在内核输入层拦截按键，不依赖于特定的桌面环境（如 GNOME/KDE）或显示协议（X11/Wayland），甚至可以在控制台 (TTY) 中运行。
- **幽灵文字 (Phantom Text)**：创新的行内实时预览模式。输入拼音时，首选词直接显示在当前光标处，切换候选词时实时更新，提供零延迟的沉浸式输入体验。
- **语义辅助码 (Semantic Auxiliary)**：通过汉字的英文释义进行二次筛选。例如输入 `li` 后按 `Shift+I` (Inside)，即可快速定位到“里”，大幅减少重码翻页。
- **极速粘贴上屏**：利用系统剪贴板实现字符注入，彻底消除在 Wayland 或高延迟环境下模拟大量 Unicode 按键序列的卡顿感。
- **多语言与方案支持**：支持拼音、日语五十音等多种输入方案，可根据配置轻松扩展。
- **高度可定制**：通过 `config.json` 轻松配置词库、快捷键和注入模式。

## 快速开始

### 1. 安装系统依赖

```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install libxcb-composite0-dev libx11-dev libdbus-1-dev
```

### 2. 配置权限 (免 sudo 运行)

1. **加入 `input` 组**：
   ```bash
   sudo usermod -aG input $USER
   ```

2. **配置 `uinput` 权限**：
   ```bash
   echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' | sudo tee /etc/udev/rules.d/99-uinput.rules
   ```

3. **重新登录** 以使组权限生效。

### 3. 编译与运行

```bash
cargo build --release
./target/release/blind-ime
```

## 快捷键说明

| 快捷键 | 功能 | 说明 |
| :--- | :--- | :--- |
| **Ctrl + Space** | 切换中/英文模式 | 开启或关闭输入法 |
| **Ctrl + Alt + P** | 切换预览模式 | 开启/关闭幽灵文字预览 |
| **Ctrl + Alt + F** | 切换模糊拼音 | 开启平卷舌/前后鼻音自动纠错 |
| **Ctrl + Alt + S** | 切换配置方案 | 在 Chinese / Japanese 等配置间切换 |
| **Ctrl + Alt + V** | 切换粘贴模式 | 适配不同终端 (Ctrl+V / Ctrl+Shift+V / Shift+Ins) |
| **Tab** | 切换候选词 | 选中下一个候选词 |
| **- / =** | 翻页 | 上一页 / 下一页 |
| **Shift + [A-Z]** | 语义过滤 | 通过按住 Shift 输入英文首字母过滤候选词 |

## 辅助码使用示例

1. **输入拼音**：输入 `li`。
2. **触发筛选**：如果你想要“里”，其对应的英文标签包含 **I**nside。
3. **输入辅助码**：按住 Shift 输入 `I`。
4. **结果**：候选词列表将只显示英文标签以 "I" 开头的字。
5. **上屏**：按空格上屏。

## 服务管理

- **安装自动启动**：`./target/release/blind-ime --install`
- **停止后台进程**：`./target/release/blind-ime --stop`
- **查看运行日志**：`cat /tmp/blind-ime.log`

## 许可证

MIT