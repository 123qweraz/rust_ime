use crate::processor::{Action, Command};
use crate::EngineContext;
use std::sync::Arc;

pub fn execute_command(ctx: &mut EngineContext, cmd: Command) -> Action {
    let page_size = ctx.config.page_size();
    match cmd {
        Command::NextPage => {
            let old_page = ctx.session.page;
            ctx.session.next_page(page_size);
            if ctx.session.page == old_page && !ctx.session.candidates.is_empty() {
                ctx.session.next_page(page_size);
            }
            Action::Consume
        }
        Command::PrevPage => {
            ctx.session.prev_page(page_size);
            Action::Consume
        }
        Command::NextCandidate => {
            let old_sel = ctx.session.selected;
            ctx.session.next_candidate(page_size);
            if ctx.session.selected == old_sel && !ctx.session.candidates.is_empty() {
                ctx.session.next_candidate(page_size);
            }
            crate::compositor::Compositor::update_phantom_action(ctx)
        }
        Command::PrevCandidate => {
            ctx.session.prev_candidate(page_size);
            crate::compositor::Compositor::update_phantom_action(ctx)
        }
        Command::Select(idx) => {
            let abs_idx = ctx.session.page + idx;
            if let Some(cand) = ctx.session.candidates.get(abs_idx) {
                let word = cand.text.clone();
                return commit_candidate(ctx, word, abs_idx);
            }
            Action::Consume
        }
        Command::Commit => {
            if ctx.session.buffer.is_empty() {
                return Action::PassThrough;
            }

            if !ctx.session.candidates.is_empty() {
                let idx = ctx.session.selected;
                if let Some(cand) = ctx.session.candidates.get(idx) {
                    let word = cand.text.clone();
                    return commit_candidate(ctx, word, idx);
                }
            }

            let out = Arc::from(ctx.session.buffer.as_str());
            commit_candidate(ctx, out, 99)
        }
        Command::CommitRaw => {
            if ctx.session.buffer.is_empty() {
                return Action::PassThrough;
            }
            let out = Arc::from(ctx.session.buffer.as_str());
            commit_candidate(ctx, out, 99)
        }
        Command::Clear => {
            ctx.session_state.commit_history.clear();
            let del = ctx.session.phantom_text.chars().count();
            ctx.reset();
            if del > 0 {
                Action::DeleteAndEmit {
                    delete: del,
                    insert: "".into(),
                }
            } else {
                Action::Consume
            }
        }
    }
}

pub(crate) fn commit_candidate(
    ctx: &mut EngineContext,
    mut cand: Arc<str>,
    index: usize,
) -> Action {
    use std::time::{Duration, Instant};

    let now = Instant::now();
    let py = ctx.session.last_lookup_pinyin.clone();

    if !py.is_empty() && index != 99 {
        if now.duration_since(ctx.session_state.last_commit_time) > Duration::from_secs(3) {
            ctx.session_state.commit_history.clear();
        }

        let last_word_opt = ctx.session_state.get_last_word().map(|s| s.to_string());
        record_usage(ctx, &py, &cand, last_word_opt.as_deref());
        ctx.session_state
            .add_to_history(py.clone(), cand.to_string());

        for (py_c, word_c) in ctx.session_state.get_combination_candidates(8) {
            record_usage(ctx, &py_c, &word_c, None);
        }
        ctx.session_state.update_commit_time();
    }

    if ctx.session_state.is_english_mode()
        && !cand.is_empty()
        && cand.chars().last().unwrap_or(' ').is_alphanumeric()
    {
        let mut s = cand.to_string();
        s.push(' ');
        cand = Arc::from(s);
    }

    let del = ctx.session.phantom_text.chars().count();
    ctx.session.clear_composing();
    Action::DeleteAndEmit {
        delete: del,
        insert: cand.to_string(),
    }
}

fn record_usage(ctx: &mut EngineContext, pinyin: &str, word: &str, context: Option<&str>) {
    use crate::processor::learning;

    if pinyin.is_empty() || word.is_empty() {
        return;
    }

    let profile = ctx.session_state.get_current_profile();
    let word_len = word.chars().count();

    if ctx.config.enable_auto_reorder() {
        let updated =
            learning::update_mru(&ctx.config.usage_history, &profile, pinyin, word, false);
        ctx.config.insert_usage(&profile, pinyin, &updated);
        ctx.engine.clear_cache();
    }

    if ctx.config.enable_auto_reorder() {
        if let Some(ctx_str) = context {
            let updated =
                learning::update_mru(&ctx.config.ngram_history, &profile, ctx_str, word, false);
            ctx.config.insert_ngram(&profile, ctx_str, &updated);
        }
    }

    if ctx.config.master_config.input.enable_word_discovery && word_len > 1 {
        if !ctx.engine.has_exact_match(&profile, pinyin, word) {
            let updated =
                learning::update_mru(&ctx.config.learned_words, &profile, pinyin, word, true);
            ctx.config.insert_learned(&profile, pinyin, &updated);
        }
    }
}
