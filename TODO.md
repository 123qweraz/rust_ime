# Rust-IME 项目进阶开发建议 (TODO)

本文件记录了项目未来的优化方向与功能建议。

---

## 1. 核心体验优化 (Experience)
- [ ] **动态词频调整 (Dynamic Frequency)**: 
    - 记录用户选择习惯，将高频选用的词置顶。
    - 实现简单的用户词库，保存新造词。
- [ ] **词组联想 (N-gram/Association)**:
    - 实现 Bigram 模型，输入“物理”后自动推荐“实验”、“现象”等关联词。
- [ ] **模糊音支持 (Fuzzy Pinyin)**:
    - 支持 `n/l`、`z/zh`、`s/sh` 等常见模糊音匹配。
- [ ] **智能纠错 (Error Correction)**:
    - 对输入过程中的字母易位或漏打进行容错处理（如 `shuaig` -> `帅哥`）。

## 2. 词库与翻译功能 (Dictionary & Translation)
- [ ] **翻译助手模式 (Dictionary Mode)**:
    - 在候选框中实时显示当前汉字的英文解释（利用已有的 `ai_translated` 词库）。
    - 增加“中译英”快捷切换，输入汉字拼音直接输出英文单词。
- [ ] **全量词库翻译**:
    - 利用 `tools/translate_all_local.py` 完成所有 `untranslated/` 目录下词汇的本地 AI 翻译。

## 3. 技术架构与环境适配 (Architecture)
- [ ] **Wayland 深度适配**:
    - 研究并接入 `text-input-v3` 或 `Input Method Portal` 协议。
    - 解决 Wayland 环境下输入框跟随光标 (Follow Cursor) 的精准定位问题。
- [ ] **插件化词库**:
    - 支持用户动态加载/卸载不同领域的专业词库（如：医学、法律、二次元等）。

## 4. 交互与美化 (UI/UX)
- [ ] **现代化候选窗口**:
    - 使用 `iced`、`egui` 或轻量级绘图库重构 UI。
    - 支持圆角、半透明、皮肤主题更换。
- [ ] **快捷指令 (Snippets)**:
    - `;time` -> 自动输入当前时间。
    - `;addr` -> 自动输入预设地址。

## 5. 性能与底层 (Low Level)
- [ ] **Trie 树优化**:
    - 进一步优化海量词库下的搜索性能和内存占用。
    - 考虑引入更高效的序列化方案加载词库。

---
*上次更新日期：2026-01-27*
