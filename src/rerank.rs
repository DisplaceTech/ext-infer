//! `Displace\Infer\RerankModel` — second-stage relevance scoring.
//!
//! ```php
//! $reranker = \Displace\Infer\RerankModel::load('models/Qwen3-Reranker-0.6B-Q8_0.gguf');
//!
//! $score = $reranker->score('how do I reset my password?', $documentText);
//!
//! $rows = $reranker->rank($query, $candidateTexts, topK: 5);
//! // [['index' => 2, 'score' => 0.93], ['index' => 0, 'score' => 0.41], ...]
//! ```
//!
//! ## How scoring works
//!
//! Qwen3-Reranker is a causal LM fine-tuned to answer a fixed yes/no
//! judgment prompt. We render the model's documented template around the
//! (query, document) pair, decode it, and read the logits of the *next*
//! token: the relevance score is the binary softmax
//! `P(yes) / (P(yes) + P(no))` — a calibrated value in (0, 1), higher is
//! more relevant.
//!
//! The prompt template is the one published on the Qwen3-Reranker model
//! card, including the empty `<think></think>` block that pins the
//! non-thinking judgment mode. It is deliberately hard-coded: reranker
//! scoring is model-family-specific by nature, and this class targets the
//! Qwen3-Reranker GGUFs (`0.6B` / `4B` / `8B`). A model whose vocabulary
//! cannot express the template's single-token `yes` / `no` answers is
//! rejected at load time.
//!
//! `rank()`'s rows are deliberately shaped like
//! `Displace\AI\Contracts\Reranker::rerank()` (best-first
//! `['index' => int, 'score' => float]`), so a contracts adapter is a
//! pass-through.
//
// `topK` is camelCase for the same reason as model.rs's parameters: the
// Rust ident *is* the PHP named-argument name, and the proc-macro moves it
// into generated code a per-method `#[allow]` can't reach.
#![allow(non_snake_case)]

use std::path::PathBuf;

use ext_php_rs::boxed::ZBox;
use ext_php_rs::prelude::*;
use ext_php_rs::types::ZendHashTable;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::token::LlamaToken;

use crate::error::InferError;
use crate::model::{backend, get_bool, get_string, get_uint};

/// The instruction baked into the judgment prompt when the caller doesn't
/// supply one — the default the Qwen3-Reranker card trains against.
const DEFAULT_INSTRUCTION: &str =
    "Given a web search query, retrieve relevant passages that answer the query";

const DEFAULT_N_CTX: u32 = 4096;

/// PHP-visible handle to a loaded reranker GGUF.
#[php_class]
#[php(name = "Displace\\Infer\\RerankModel")]
#[derive(Default)]
pub struct RerankModel {
    inner: Option<LlamaModel>,
    instruction: String,
    n_ctx: u32,
    yes_token: i32,
    no_token: i32,
}

#[php_impl]
impl RerankModel {
    /// Direct construction is not supported — use `RerankModel::load()`.
    pub fn __construct() -> PhpResult<Self> {
        Err(InferError::InvalidConstruction(
            "use Displace\\Infer\\RerankModel::load() to construct a RerankModel".into(),
        )
        .into())
    }

    /// Load a reranker GGUF (Qwen3-Reranker family) from disk.
    ///
    /// Recognised `$options` keys:
    /// - `n_gpu_layers` (int, default 0)
    /// - `use_mmap` (bool, default true)
    /// - `use_mlock` (bool, default false)
    /// - `n_ctx` (int, default 4096) — context size per scoring call; the
    ///   rendered template plus query plus document must fit.
    /// - `instruction` (string) — the task instruction embedded in the
    ///   judgment prompt. The default is the generic web-search retrieval
    ///   instruction from the model card; tailoring it to the corpus
    ///   ("Given a support question, retrieve KB articles that resolve
    ///   it") measurably helps.
    pub fn load(path: String, options: Option<&ZendHashTable>) -> PhpResult<Self> {
        let n_gpu_layers = get_uint(options, "n_gpu_layers")?.unwrap_or(0);
        let use_mmap = get_bool(options, "use_mmap")?.unwrap_or(true);
        let use_mlock = get_bool(options, "use_mlock")?.unwrap_or(false);
        let n_ctx = get_uint(options, "n_ctx")?.unwrap_or(DEFAULT_N_CTX);
        let instruction =
            get_string(options, "instruction")?.unwrap_or_else(|| DEFAULT_INSTRUCTION.into());

        if n_ctx == 0 {
            return Err(InferError::InvalidOption {
                name: "n_ctx".into(),
                reason: "must be greater than zero".into(),
            }
            .into());
        }

        let path_buf = PathBuf::from(&path);
        if !path_buf.is_file() {
            return Err(InferError::ModelLoad(format!("no such file: {path}")).into());
        }

        let backend = backend()?;
        let params = LlamaModelParams::default()
            .with_n_gpu_layers(n_gpu_layers)
            .with_use_mmap(use_mmap)
            .with_use_mlock(use_mlock);
        let model = LlamaModel::load_from_file(backend, &path_buf, &params)
            .map_err(|e| InferError::ModelLoad(format!("{path}: {e}")))?;

        // The scoring scheme reads the logits of single-token "yes" / "no"
        // answers. Resolve those ids once; a vocabulary where either is
        // not a single token cannot be scored this way, so refuse it now
        // with a useful message instead of mis-scoring later.
        let yes_token = single_token(&model, "yes")?;
        let no_token = single_token(&model, "no")?;

        Ok(Self {
            inner: Some(model),
            instruction,
            n_ctx,
            yes_token,
            no_token,
        })
    }

    /// Relevance of `document` to `query`, in (0, 1) — the binary softmax
    /// over the model's yes/no judgment logits. Higher is more relevant;
    /// scores are calibrated enough to threshold (e.g. "drop candidates
    /// under 0.3").
    pub fn score(&self, query: String, document: String) -> PhpResult<f64> {
        let model = self.inner.as_ref().ok_or(InferError::Closed)?;
        Ok(self.run_score(model, &query, &document)?)
    }

    /// Score every document against the query and return best-first rows.
    ///
    /// Each row is `['index' => int, 'score' => float]`, where `index` is
    /// the document's position in the input list — the same shape as
    /// `Displace\AI\Contracts\Reranker::rerank()`. Ties keep input order.
    ///
    /// @param list<string> $documents
    #[php(defaults(topK = None))]
    pub fn rank(
        &self,
        query: String,
        documents: Vec<String>,
        topK: Option<i64>,
    ) -> PhpResult<Vec<ZBox<ZendHashTable>>> {
        let model = self.inner.as_ref().ok_or(InferError::Closed)?;

        let keep = match topK {
            None => documents.len(),
            Some(k) if k >= 1 => usize::try_from(k).unwrap_or(documents.len()),
            Some(k) => {
                return Err(InferError::InvalidOption {
                    name: "topK".into(),
                    reason: format!("must be at least 1, got {k}"),
                }
                .into());
            }
        };

        let mut scored: Vec<(usize, f64)> = Vec::with_capacity(documents.len());
        for (index, document) in documents.iter().enumerate() {
            scored.push((index, self.run_score(model, &query, document)?));
        }
        // Best-first; equal scores keep input order (stable sort).
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(keep);

        let mut rows = Vec::with_capacity(scored.len());
        for (index, score) in scored {
            let mut row = ZendHashTable::new();
            row.insert("index", index as i64)?;
            row.insert("score", score)?;
            rows.push(row);
        }
        Ok(rows)
    }

    /// Release the underlying model weights. Idempotent.
    pub fn close(&mut self) {
        self.inner = None;
    }
}

impl RerankModel {
    /// Render the judgment prompt, decode it, and read the yes/no logits.
    fn run_score(
        &self,
        model: &LlamaModel,
        query: &str,
        document: &str,
    ) -> Result<f64, InferError> {
        // The Qwen3-Reranker prompt format, verbatim from the model card.
        // The empty <think> block pins non-thinking judgment mode; the
        // assistant turn is left open so the next token *is* the verdict.
        let prompt = format!(
            "<|im_start|>system\nJudge whether the Document meets the requirements based on the \
             Query and the Instruct provided. Note that the answer can only be \"yes\" or \
             \"no\".<|im_end|>\n<|im_start|>user\n<Instruct>: {}\n<Query>: {}\n<Document>: \
             {}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n",
            self.instruction, query, document,
        );

        let backend = backend()?;
        let n_ctx = std::num::NonZeroU32::new(self.n_ctx).expect("validated at load");
        let ctx_params = LlamaContextParams::default().with_n_ctx(Some(n_ctx));
        let mut ctx = model
            .new_context(backend, ctx_params)
            .map_err(|e| InferError::Inference(format!("rerank context creation failed: {e}")))?;

        // The template carries its own <|im_start|> framing — no BOS.
        let tokens = model
            .str_to_token(&prompt, AddBos::Never)
            .map_err(|e| InferError::Inference(format!("tokenization failed: {e}")))?;
        let token_count = i32::try_from(tokens.len())
            .map_err(|_| InferError::Inference("prompt token count overflows i32".into()))?;
        if tokens.is_empty() {
            return Err(InferError::Inference("empty rerank prompt".into()));
        }
        if (token_count as u32) > self.n_ctx {
            return Err(InferError::Inference(format!(
                "query + document render to {token_count} tokens but n_ctx is {} — \
                 raise ['n_ctx' => ...] at load time or chunk the document",
                self.n_ctx
            )));
        }

        let mut batch = LlamaBatch::new(tokens.len(), 1);
        let last_index = token_count - 1;
        for (i, token) in tokens.into_iter().enumerate() {
            let i = i as i32;
            batch
                .add(token, i, &[0], i == last_index)
                .map_err(|e| InferError::Inference(format!("batch add failed: {e}")))?;
        }
        ctx.decode(&mut batch)
            .map_err(|e| InferError::Inference(format!("rerank decode failed: {e}")))?;

        let logits = ctx.get_logits_ith(batch.n_tokens() - 1);
        let yes = f64::from(logits[self.yes_token as usize]);
        let no = f64::from(logits[self.no_token as usize]);

        // Binary softmax, stabilized so large logits can't overflow exp().
        let max = yes.max(no);
        let e_yes = (yes - max).exp();
        let e_no = (no - max).exp();
        Ok(e_yes / (e_yes + e_no))
    }
}

/// Resolve `text` to exactly one vocabulary token id, or explain why this
/// model can't be used for yes/no logit scoring.
fn single_token(model: &LlamaModel, text: &str) -> Result<i32, InferError> {
    let tokens: Vec<LlamaToken> = model
        .str_to_token(text, AddBos::Never)
        .map_err(|e| InferError::ModelLoad(format!("cannot tokenize {text:?}: {e}")))?;
    match tokens.as_slice() {
        [only] => Ok(only.0),
        other => Err(InferError::ModelLoad(format!(
            "model is not usable as a yes/no reranker: {text:?} tokenizes to {} tokens \
             (expected 1) — RerankModel targets the Qwen3-Reranker GGUF family",
            other.len()
        ))),
    }
}
