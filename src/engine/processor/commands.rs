use crate::engine::processor::{Action, Command, Processor};
use std::sync::Arc;

pub fn execute_command(processor: &mut Processor, cmd: Command) -> Action {
    let page_size = processor.config.page_size;
    match cmd {
        Command::NextPage => {
            let old_page = processor.session.page;
            processor.session.next_page(page_size);
            if processor.session.page == old_page && !processor.session.candidates.is_empty() {
                processor.trigger_incremental_search();
                processor.session.next_page(page_size);
            }
            Action::Consume
        }
        Command::PrevPage => {
            processor.session.prev_page(page_size);
            Action::Consume
        }
        Command::NextCandidate => {
            let old_sel = processor.session.selected;
            processor.session.next_candidate(page_size);
            if processor.session.selected == old_sel && !processor.session.candidates.is_empty() {
                processor.trigger_incremental_search();
                processor.session.next_candidate(page_size);
            }
            processor.update_phantom_action()
        }
        Command::PrevCandidate => {
            processor.session.prev_candidate(page_size);
            processor.update_phantom_action()
        }
        Command::Select(idx) => {
            let abs_idx = processor.session.page + idx;
            if let Some(cand) = processor.session.candidates.get(abs_idx) {
                let word = cand.text.clone();
                return processor.commit_candidate(word, abs_idx);
            }
            Action::Consume
        }
        Command::Commit => {
            if processor.session.buffer.is_empty() {
                return Action::PassThrough;
            }

            // 优先尝试提交当前选中的候选词
            if !processor.session.candidates.is_empty() {
                let idx = processor.session.selected;
                if let Some(cand) = processor.session.candidates.get(idx) {
                    let word = cand.text.clone();
                    return processor.commit_candidate(word, idx);
                }
            }

            // 如果完全没有候选词，才提交原始 buffer (例如未知输入)
            let out = Arc::from(processor.session.buffer.as_str());
            processor.commit_candidate(out, 99)
        }
        Command::CommitRaw => {
            if processor.session.buffer.is_empty() {
                return Action::PassThrough;
            }
            let out = Arc::from(processor.session.buffer.as_str());
            processor.commit_candidate(out, 99)
        }
        Command::Clear => {
            processor.session_state.commit_history.clear();
            let del = processor.session.phantom_text.chars().count();
            processor.reset();
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
