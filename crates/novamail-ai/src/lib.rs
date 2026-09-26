//! Local-first AI adapters.
//!
//! ADR 0004: providers perform real work. `NullAiProvider` is an explicit
//! offline fallback (deterministic extractive summary), not fabricated mail.

mod ollama;
mod provider;

pub use ollama::OllamaProvider;
pub use provider::{
    AiError, AiProvider, AiResult, NullAiProvider, PrioritizeRequest, PrioritizeResponse,
    SuggestReplyRequest, SuggestReplyResponse, SummarizeRequest, SummarizeResponse,
};
