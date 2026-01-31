# Rust-IME Future Roadmap

## 1. 跨平台支持
- [ ] **Wayland 原生协议集成**: 
    - 使用 `zwp_input_method_v2` 实现真正的内联预编辑和光标跟随。
    - 解决 `wayland-client` 0.31 的协议代码隔离与冲突问题。
- [ ] **Windows (TSF) 支持**:
    - 实现基于 Windows Text Services Framework 的 Host。
    - 探索使用 Win32 API 拦截按键作为兼容模式。

## 2. 核心功能增强
- [ ] **光标跟随 (KDE/Wayland 临时方案)**: 
    - 探索通过 DBus 实时获取 KWin 光标坐标来移动当前 GTK 候选窗。
- [ ] **云同步**: 
    - 支持用户词库的云端备份与同步。
- [ ] **动态皮肤**: 
    - 在 Web 配置中心增加主题编辑器，允许自定义候选框 CSS。

## 3. 性能优化
- [ ] **词库索引优化**: 
    - 进一步压缩 Trie 索引，提升冷启动速度。
- [ ] **N-Gram 模型微调**: 
    - 增加用户习惯学习频率，提高首选词准确率。
