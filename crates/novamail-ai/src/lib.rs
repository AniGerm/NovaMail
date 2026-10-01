//! Local-first AI adapters.
//!
//! ADR 0004: providers perform real work. `NullAiProvider` is an explicit
//! offline fallback (deterministic extractive summary), not fabricated mail.

mod install;
mod language;
mod ollama;
mod provider;
mod runtime;

pub use install::{
    ensure_ollama_running, install_ollama_system, install_ollama_user, probe_ollama,
    InstallProgress, OllamaPresence,
};
pub use language::{language_label, language_native_name, resolve_output_language, LangTag};
pub use ollama::OllamaProvider;
pub use provider::{
    heuristic_event_suggestions, strip_quoted_reply, AiError, AiProvider, AiResult, EventSuggestion,
    ExtractEventsRequest, ExtractEventsResponse, NullAiProvider, PrioritizeRequest,
    PrioritizeResponse, SuggestReplyRequest, SuggestReplyResponse, SuggestReplyVariantsResponse,
    SummarizeRequest, SummarizeResponse,
};
pub use runtime::{
    allow_model_pick, default_ollama_model, default_ollama_url, detect_nvidia_gpu,
    list_ollama_models, normalize_model_ref, ollama_reachable, pull_ollama_model,
    recommended_model, suggested_models, CPU_DEFAULT_MODEL, DEFAULT_MODEL, PullProgress,
};
