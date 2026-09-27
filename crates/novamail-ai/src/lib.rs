//! Local-first AI adapters.
//!
//! ADR 0004: providers perform real work. `NullAiProvider` is an explicit
//! offline fallback (deterministic extractive summary), not fabricated mail.

mod ollama;
mod provider;
mod runtime;

pub use ollama::OllamaProvider;
pub use provider::{
    AiError, AiProvider, AiResult, NullAiProvider, PrioritizeRequest, PrioritizeResponse,
    SuggestReplyRequest, SuggestReplyResponse, SuggestReplyVariantsResponse, SummarizeRequest,
    SummarizeResponse,
};
pub use runtime::{
    allow_model_pick, default_ollama_model, default_ollama_url, detect_nvidia_gpu,
    list_ollama_models, normalize_model_ref, ollama_reachable, pull_ollama_model,
    recommended_model, suggested_models, CPU_DEFAULT_MODEL, DEFAULT_MODEL, PullProgress,
};
