//! `Displace\Infer\Response` — the return value of `Model::chat()`.
//!
//! Conceptually a triple of `(reasoning, answer, metadata)` derived from the
//! raw text the model emitted. The split between reasoning and answer is the
//! `<think>...</think>` convention shared by Qwen3, DeepSeek R1, and other
//! "thinking" models; for models that don't emit those tags, `reasoning()`
//! returns `null` and `answer()` is the same as `text()`.
//!
//! Reasoning is parsed once at construction time so repeated calls to
//! `reasoning()` / `answer()` from PHP don't re-scan the string.
//
// `Response::new`, `split_thinking`, and the `FinishReason` variants
// are crate-private helpers that get exercised the moment `Model::chat()`
// (the next commit) is wired up. They are reachable from the `#[cfg(test)]`
// suite below but not from `cargo build` output, so silence the
// `dead_code` lint until the consumer lands.
#![allow(dead_code)]

use ext_php_rs::prelude::*;

use crate::error::InferError;

/// Why generation stopped. Surfaced to PHP as the result of
/// `Response::finishReason()`.
#[derive(Debug, Clone, Copy)]
pub(crate) enum FinishReason {
    /// Sampling produced an end-of-generation token from the model.
    Eos,
    /// `max_tokens` was reached before the model emitted EOS.
    Length,
    /// Generation completed without hitting either of the above — e.g. an
    /// empty prompt that produced no tokens. (Reserved for future stop-string
    /// support.)
    Stop,
}

impl FinishReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Eos => "eos",
            Self::Length => "length",
            Self::Stop => "stop",
        }
    }
}

/// Result of a chat completion.
///
/// Constructed internally by `Model::chat()`. PHP `new Response()` is
/// refused — there's nothing useful you can put in one from PHP, and a
/// hand-rolled instance would lie about the underlying model state.
#[php_class]
#[php(name = "Displace\\Infer\\Response")]
#[derive(Default, Clone)]
pub struct Response {
    text: String,
    reasoning: Option<String>,
    answer: String,
    finish_reason: &'static str,
    tokens_generated: u32,
}

#[php_impl]
impl Response {
    /// Refuse direct construction.
    pub fn __construct() -> PhpResult<Self> {
        Err(InferError::InvalidConstruction(
            "Displace\\Infer\\Response is produced by Model::chat(); do not instantiate directly"
                .into(),
        )
        .into())
    }

    /// The full text the model emitted, including any `<think>` block(s).
    /// Equivalent to a raw completion's return value.
    pub fn text(&self) -> String {
        self.text.clone()
    }

    /// Content of the `<think>...</think>` block(s), with the tags removed
    /// and multiple blocks joined by a blank line. Returns `null` for models
    /// that don't emit reasoning (most non-reasoning models, or reasoning
    /// models invoked through `/no_think`).
    pub fn reasoning(&self) -> Option<String> {
        self.reasoning.clone()
    }

    /// `text()` with `<think>...</think>` block(s) removed and the leading
    /// whitespace the model habitually emits before its answer trimmed.
    /// For non-reasoning outputs this is byte-identical to `text()`.
    pub fn answer(&self) -> String {
        self.answer.clone()
    }

    /// `true` if any `<think>...</think>` block was present in the model's
    /// raw output.
    pub fn has_reasoning(&self) -> bool {
        self.reasoning.is_some()
    }

    /// Why generation stopped: one of `'eos'`, `'length'`, or `'stop'`.
    pub fn finish_reason(&self) -> String {
        self.finish_reason.to_string()
    }

    /// Number of tokens the model generated (the prompt's tokens are not
    /// counted).
    pub fn tokens_generated(&self) -> u32 {
        self.tokens_generated
    }
}

impl Response {
    /// Build a `Response` from raw inference output. Splits the reasoning
    /// from the answer at construction time so PHP getters are cheap.
    pub(crate) fn new(text: String, finish_reason: FinishReason, tokens_generated: u32) -> Self {
        let (reasoning, answer) = split_thinking(&text);
        Self {
            text,
            reasoning,
            answer,
            finish_reason: finish_reason.as_str(),
            tokens_generated,
        }
    }
}

/// Pull `<think>...</think>` blocks out of model output, returning
/// `(reasoning, answer)`.
///
/// - Every complete `<think>...</think>` block is captured. Multiple blocks
///   are joined into the reasoning with a blank line so the `reasoning()`
///   getter returns something coherent rather than concatenated globs.
/// - The answer is the input minus those blocks. When at least one block
///   was removed, leading whitespace is trimmed because the closing tag is
///   reliably followed by `"\n\n"` before the model's actual reply. When no
///   tag was present the input is returned untouched — `answer()` on a
///   non-reasoning output must equal `text()` byte-for-byte.
/// - An unclosed `<think>` (typical when `max_tokens` truncates the run
///   mid-thought) is left in the answer verbatim; previously captured
///   blocks are still returned as the reasoning so the caller sees what
///   thinking the model did get to finish.
pub(crate) fn split_thinking(text: &str) -> (Option<String>, String) {
    const OPEN: &str = "<think>";
    const CLOSE: &str = "</think>";

    let mut reasoning_parts: Vec<String> = Vec::new();
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find(OPEN) {
        out.push_str(&rest[..start]);
        let after_open = &rest[start + OPEN.len()..];
        match after_open.find(CLOSE) {
            Some(end) => {
                reasoning_parts.push(after_open[..end].to_string());
                rest = &after_open[end + CLOSE.len()..];
            }
            None => {
                out.push_str(&rest[start..]);
                let reasoning = (!reasoning_parts.is_empty()).then(|| reasoning_parts.join("\n\n"));
                return (reasoning, out);
            }
        }
    }
    out.push_str(rest);

    let answer = if reasoning_parts.is_empty() {
        out
    } else {
        out.trim_start().to_string()
    };
    let reasoning = (!reasoning_parts.is_empty()).then(|| reasoning_parts.join("\n\n"));
    (reasoning, answer)
}

#[cfg(test)]
mod tests {
    use super::{split_thinking, FinishReason, Response};

    #[test]
    fn split_returns_none_and_original_when_no_tags() {
        let (reasoning, answer) = split_thinking("  hello world");
        assert_eq!(reasoning, None);
        // Whitespace preserved when nothing was stripped.
        assert_eq!(answer, "  hello world");
    }

    #[test]
    fn split_extracts_single_block_and_trims_answer() {
        let raw = "<think>Okay so 2+2 = 4.</think>\n\n2 + 2 = 4.";
        let (reasoning, answer) = split_thinking(raw);
        assert_eq!(reasoning.as_deref(), Some("Okay so 2+2 = 4."));
        assert_eq!(answer, "2 + 2 = 4.");
    }

    #[test]
    fn split_joins_multiple_blocks_with_blank_line() {
        let raw = "<think>first</think>mid<think>second</think>end";
        let (reasoning, answer) = split_thinking(raw);
        assert_eq!(reasoning.as_deref(), Some("first\n\nsecond"));
        assert_eq!(answer, "midend");
    }

    #[test]
    fn split_returns_captured_blocks_and_partial_tail_when_open_unclosed() {
        let raw = "<think>step one</think>\n\nintermediate<think>step two";
        let (reasoning, answer) = split_thinking(raw);
        // First block captured; second block was truncated so its
        // partial content stays in the answer rather than disappearing.
        assert_eq!(reasoning.as_deref(), Some("step one"));
        assert_eq!(answer, "\n\nintermediate<think>step two");
    }

    #[test]
    fn split_treats_orphan_close_as_literal() {
        let raw = "answer </think> trailing";
        let (reasoning, answer) = split_thinking(raw);
        assert_eq!(reasoning, None);
        assert_eq!(answer, "answer </think> trailing");
    }

    #[test]
    fn response_new_populates_all_fields() {
        let r = Response::new(
            "<think>thought</think>\n\nresult".to_string(),
            FinishReason::Eos,
            17,
        );
        assert_eq!(r.text, "<think>thought</think>\n\nresult");
        assert_eq!(r.reasoning.as_deref(), Some("thought"));
        assert_eq!(r.answer, "result");
        assert_eq!(r.finish_reason, "eos");
        assert_eq!(r.tokens_generated, 17);
    }
}
