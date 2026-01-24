# Rust-IME

一个基于 Rust 开发的 Linux 系统级输入法框架。通过监听底层键盘事件 (`/dev/input/event`) 并利用虚拟键盘 (uinput) 注入字符，实现与显示协议（Wayland/X11/TTY）无关的全局输入能力。

## 🚀 特点

- **底层驱动级监听**：在内核输入层拦截按键，不依赖特定桌面环境或显示协议，支持在 Wayland、X11 甚至纯终端 (TTY) 中完美运行。
- **幽灵文字 (Phantom Text)**：独创的行内实时预览模式，首选词直接显示在当前光标处，提供零延迟的沉浸式输入体验。
- **语义辅助码 (Semantic Auxiliary)**：通过汉字的英文标签进行快速过滤（例如输入 `li` 后按 `Shift+I` 即可定位到“里” Inside），大幅降低重码率。
- **批量拼音转换**：选中已输入的拼音串，一键转换为汉字，适合长句输入后的快速修正。
- **系统托盘集成**：实时显示当前输入模式与方案，支持菜单快速切换。
- **极速粘贴注入**：利用系统剪贴板实现字符上屏，彻底解决 Wayland 下模拟 Unicode 按键序列产生的卡顿。

---

## 📦 安装教程

### 1. 安装系统依赖

```bash
# Ubuntu/Debian 示例
sudo apt-get update
sudo apt-get install -y libxcb-composite0-dev libx11-dev libdbus-1-dev pkg-config build-essential
```

### 2. 配置硬件访问权限 (免 sudo)

1. **加入输入设备组**：
   ```bash
   sudo usermod -aG input $USER
   sudo usermod -aG uinput $USER
   ```
2. **配置 uinput 规则**：
   ```bash
   echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' | sudo tee /etc/udev/rules.d/99-rust-ime-uinput.rules
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```
3. **重要**：完成上述步骤后，请**注销并重新登录**（或注销会话）以使组权限生效。

### 3. 构建与运行

```bash
cargo build --release
./target/release/rust-ime --help
```

---

## 🛠 使用教程

### 快捷键说明 (默认配置)

| 快捷键 | 功能 | 说明 |
| :--- | :--- | :--- |
| **CapsLock** / **Ctrl+Space** | 切换输入法 | 开启/关闭中英文输入模式 |
| **Ctrl + R** | 拼音转汉字 | **[新]** 将选中的拼音串（如 `nihao`）转换为汉字 |
| **Ctrl + Alt + S** | 切换方案 | 在中文、日语等不同输入 Profile 间切换 |
| **Tab** | 切换候选词 | 在备选列表中循环选择 |
| **- / =** | 翻页 | 上一页 / 下一页候选词 |
| **Space** | 确认上屏 | 将首选词或选中词输入到当前位置 |
| **Enter** | 原始输入 | 直接输入当前缓存的拼音字符串 |
| **Esc** | 取消输入 | 清空当前输入缓存 |
| **Shift + [A-Z]** | 语义过滤 | 在输入拼音后，按住 Shift 输入英文首字母进行筛选 |
| **Ctrl + Alt + V** | 切换粘贴模式 | 循环切换：Ctrl+V, Ctrl+Shift+V, Shift+Insert 等 |
| **Ctrl + Alt + T** | TTY 模式 | 切换直接字节注入模式（适合纯终端） |

### 辅助码示例
1. 输入拼音 `li`。
2. 想要“里” (Inside)，按住 `Shift` 并输入 `i`。
3. 候选词将立即筛选出带有 `inside` 标签的汉字。

---

## 🖥 系统托盘

程序启动后会在系统托盘显示图标：
- **图标变化**：区分中英文模式（基于系统 `keyboard` 和 `input-keyboard` 图标）。
- **右键菜单**：
    - 快速查看/切换当前模式。
    - 查看/切换当前输入方案。
    - 退出程序。

---

## 许可证
MIT License