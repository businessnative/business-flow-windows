//! Platform-independent Business Flow core.
//! Behavior is ported from the public Mac app's `BNFlowCore` so the Hungarian
//! text rules, hotkey state machine, and provider request shapes match.

pub mod audio;
pub mod autolearn;
pub mod editing;
pub mod formatter;
pub mod history;
pub mod hotkey;
pub mod latency;
pub mod meetings;
pub mod models;
pub mod prompts;
pub mod providers;
pub mod recovery;
pub mod text_pipeline;
pub mod text_rules;
pub mod util;

pub use audio::{clean_transcript, is_silent, prepare, AudioStats};
pub use autolearn::AutoLearn;
pub use editing::{DictionaryEditing, DiffOp, SnippetEditing, WordDiff, POLISH_PRESETS};
pub use history::{HistoryLogic, HistorySearch, HistoryStats, OnboardingState};
pub use recovery::{Attempt, AudioFolder, Decision, Outcome, Recovery, RetryUpdate, ACCIDENTAL_TAP_SECONDS};
pub use latency::{CallBudget, DictationTrace, PasteConfirm, PreparedTurn, SpeculationPolicy, TranscriptRequestId, TurnSelection};
pub use meetings::{Ics, MeetingAudio, MeetingCaption, MeetingCaptions, MeetingEvent, MeetingLinks, MeetingNotes, MeetingPlatform};
pub use formatter::{ChatProvider, DictationFormatter, FlowError, FormatPolicy, FormatResult, LLMGuard};
pub use hotkey::{HotkeyAction, HotkeyEngine, HotkeyState};
pub use models::*;
pub use prompts::Prompts;
pub use providers::*;
pub use text_pipeline::TextPipeline;
pub use text_rules::{AppCategoryResolver, TextRules};
