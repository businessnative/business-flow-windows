//! Platform-independent Business Flow core.
//! Behavior is ported from the public Mac app's `BNFlowCore` so the Hungarian
//! text rules, hotkey state machine, and provider request shapes match.

pub mod audio;
pub mod formatter;
pub mod hotkey;
pub mod models;
pub mod prompts;
pub mod providers;
pub mod text_pipeline;
pub mod text_rules;
pub mod util;

pub use audio::{clean_transcript, is_silent, prepare, AudioStats};
pub use formatter::{ChatProvider, DictationFormatter, FlowError, FormatPolicy, FormatResult, LLMGuard};
pub use hotkey::{HotkeyAction, HotkeyEngine, HotkeyState};
pub use models::*;
pub use prompts::Prompts;
pub use providers::*;
pub use text_pipeline::TextPipeline;
pub use text_rules::{AppCategoryResolver, TextRules};
