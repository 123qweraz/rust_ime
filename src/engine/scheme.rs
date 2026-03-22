use crate::config::Config;
use crate::engine::keys::VirtualKey;
use crate::engine::processor::{Action, FilterMode};
use crate::engine::trie::Trie;
use std::collections::HashMap;

/// 代表一个候选项的完整信息
#[derive(Debug, Clone)]
pub struct SchemeCandidate {
    pub text: String,        // 显示的候选文字（如“的”）
    pub simplified: String,  // 简体形式
    pub traditional: String, // 繁体形式
    pub tone: String,        // 声调/拼音提示
    pub english: String,     // 英文释义/提示
    pub stroke_aux: String,  // 笔画辅助码提示
    pub weight: u32,         // 排序权重
    pub match_level: u8,     // 匹配级别：3=精确, 2=简拼, 1=前缀
}

impl SchemeCandidate {
    pub fn new(text: String, weight: u32) -> Self {
        Self {
            simplified: text.clone(),
            traditional: text.clone(),
            text,
            tone: String::new(),
            english: String::new(),
            stroke_aux: String::new(),
            weight,
            match_level: 1,
        }
    }
}

use crate::engine::config_manager::UserDictData;
use arc_swap::ArcSwap;
use std::sync::Arc;

/// 方案执行时的上下文环境
pub struct SchemeContext<'a> {
    pub config: &'a Config,
    pub tries: &'a HashMap<String, Trie>,
    pub syllables: &'a std::collections::HashSet<String>,
    pub _user_dict: &'a Arc<ArcSwap<UserDictData>>,
    pub active_profiles: &'a [String],
    pub candidate_count: usize,
    pub _filter_mode: FilterMode,
    pub _aux_filter: &'a str,
}

/// 输入方案接口定义
pub trait InputScheme: Send + Sync {
    /// 获取方案唯一标识名称
    #[allow(dead_code)]
    fn name(&self) -> &str;

    /// 预处理阶段：转换输入缓冲区
    /// 例如：双拼转全拼，或者笔画数字转映射字母
    fn pre_process(&self, buffer: &str, _context: &SchemeContext) -> String {
        buffer.to_string()
    }

    /// 检索阶段：执行词库查找
    fn lookup(&self, query: &str, context: &SchemeContext) -> Vec<SchemeCandidate>;

    /// 后处理阶段：过滤、排序和修饰结果
    fn post_process(
        &self,
        query: &str,
        candidates: &mut Vec<SchemeCandidate>,
        context: &SchemeContext,
    );

    /// 处理方案特有的按键（如快捷键开关）
    fn handle_special_key(
        &self,
        _key: VirtualKey,
        _buffer: &mut String,
        _context: &SchemeContext,
    ) -> Option<Action> {
        None
    }
}
