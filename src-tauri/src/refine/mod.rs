//! Turning a raw transcript into text worth reading.
//!
//! Kept deliberately separate from `providers`: blueprint §7 requires that the
//! speech-to-text engine and the rewrite engine stay independent, so changing
//! one never silently changes the other.
//!
//! Refinement is always optional and always recoverable. If a refiner is
//! missing, switched off, or fails, the pipeline uses the transcript as spoken
//! — a rewrite that cannot run must never cost the user their words.

pub mod apple_intelligence;
pub mod registry;
pub mod traits;

pub use registry::RefinerRegistry;
pub use traits::{
    accepts_refinement, RefineError, RefineRequest, RefineStyle, Refiner, RefinerDescriptor,
};
