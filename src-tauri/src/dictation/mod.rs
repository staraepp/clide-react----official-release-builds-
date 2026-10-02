//! Dictation: the state machine, the live transaction, and the pipeline that
//! moves audio through transcription, processing, and insertion.

pub mod events;
pub mod fallback;
pub mod live;
pub mod machine;
pub mod pipeline;
pub mod session;

pub use machine::{DictationBehavior, DictationState};
pub use session::DictationSession;
