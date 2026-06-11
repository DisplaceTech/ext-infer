//! `Displace\Infer\Model` — the loaded GGUF and its inference entry points.
//!
//! The public surface is two methods plus housekeeping:
//!
//! ```php
//! $model = \Displace\Infer\Model::load('/path/to/llama.gguf');
//!
//! // Fluent chat with role-aware prompts. Renders through the model's
//! // embedded chat template — callers never see <|im_start|>.
//! $resp = $model->chat(
//!     \Displace\Infer\Prompt::system('You are helpful.')->withUser('What is 2+2?'),
//!     maxTokens: 256,
//!     temperature: 0.0,
//! );
//! echo $resp->answer();      // model's reply, with <think>...</think> stripped
//! echo $resp->reasoning();   // ?string — the stripped reasoning, or null
//!
//! // Escape hatch for callers who need full control over the prompt string.
//! $text = $model->raw('Once upon a time, ', maxTokens: 64);
//!
//! $model->close();
//! ```
//!
//! A `Model` owns a [`LlamaModel`] (the in-memory weights). Each `chat()` /
//! `raw()` call constructs a fresh [`LlamaContext`] from those weights, runs
//! a synchronous decode/sample loop, and drops the context — multiple
//! threads can call into the same `Model` concurrently because llama.cpp
//! explicitly supports many contexts on one model. KV-cache reuse across
//! calls is a future addition.
//
// `chat()` and `raw()` use camelCase parameter idents on purpose: PHP
// named-arguments echo the Rust ident verbatim, and we want
// `$model->chat($p, maxTokens: 256)` rather than `max_tokens:` for the
// public API. The proc-macro expansion shifts those idents into generated
// code where per-method `#[allow]` doesn't reach, so the lint is silenced
// at the module level instead.
#![allow(non_snake_case)]

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use ext_php_rs::convert::FromZval;
use ext_php_rs::prelude::*;
use ext_php_rs::types::ZendHashTable;

use llama_cpp_2::context::params::{LlamaContextParams, LlamaPoolingType};
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

use crate::embedding::Embedding;
use crate::error::InferError;
use crate::grammar::json_schema_to_gbnf;
use crate::prompt::Prompt;
use crate::response::{FinishReason, Response};

// --- Backend singleton ------------------------------------------------------
//
// `LlamaBackend::init()` is process-global state inside llama.cpp; calling it
// twice is undefined behavior. We initialize lazily under a mutex and hand
// out a `'static` reference. The mutex is only contended during the first
// few `Model::load()` calls; once `BACKEND` is `Some`, the fast path is a
// single atomic load.

static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
static BACKEND_INIT: Mutex<()> = Mutex::new(());

fn backend() -> Result<&'static LlamaBackend, InferError> {
    if let Some(b) = BACKEND.get() {
        return Ok(b);
    }
    let _guard = BACKEND_INIT
        .lock()
        .map_err(|_| InferError::ModelLoad("backend init mutex poisoned".into()))?;
    if let Some(b) = BACKEND.get() {
        return Ok(b);
    }
    let mut backend = LlamaBackend::init()
        .map_err(|e| InferError::ModelLoad(format!("llama backend init failed: {e}")))?;
    // llama.cpp logs profusely to stderr by default (model layout, KV cache
    // sizing, graph reservation, ...). For a PHP extension running inside a
    // web request or CLI worker, that flood is noise — silence it unless the
    // caller explicitly asks for it via `EXT_INFER_LOG=1`. `llama_log_set`
    // (called transitively from `void_logs`) is process-global, so doing
    // this once during backend init covers every subsequent llama.cpp call.
    if std::env::var_os("EXT_INFER_LOG").is_none() {
        backend.void_logs();
    }
    // `set` can only fail if another thread won the race; in that case the
    // value is already populated and we fall through to the `get().unwrap()`.
    let _ = BACKEND.set(backend);
    Ok(BACKEND.get().expect("backend just set"))
}

const BATCH_CAPACITY: usize = 512;

// --- Model ------------------------------------------------------------------

/// PHP-visible handle to a loaded GGUF model.
///
/// Internally wraps an `Option<LlamaModel>` so that `close()` can release the
/// underlying weights deterministically rather than waiting for PHP's GC.
/// After `close()`, every other method throws `InferenceException`.
#[php_class]
#[php(name = "Displace\\Infer\\Model")]
pub struct Model {
    inner: Option<LlamaModel>,
    /// Whether this handle was loaded with `embedding: true`. `embed()` checks
    /// this and refuses to run on a generation-mode handle — embedding mode
    /// requires `with_embeddings(true)` on the context, which conflicts with
    /// causal-LM decoding.
    embedding_mode: bool,
    /// Pooling strategy used when the context is built in embedding mode.
    /// `Unspecified` lets llama.cpp pick based on the GGUF's metadata —
    /// almost always the right answer for purpose-built embedding models.
    pooling_type: LlamaPoolingType,
}

// `LlamaPoolingType` doesn't derive `Default`, so we can't blanket-derive
// `Default` on `Model`. Hand-roll the impl with the same "trust GGUF
// metadata" choice the constructor's default-arm uses.
impl Default for Model {
    fn default() -> Self {
        Self {
            inner: None,
            embedding_mode: false,
            pooling_type: LlamaPoolingType::Unspecified,
        }
    }
}

#[php_impl]
impl Model {
    /// Direct construction is not supported — use `Model::load()`. This
    /// constructor exists only to give callers a clear error if they try
    /// `new Model()` out of habit.
    pub fn __construct() -> PhpResult<Self> {
        Err(InferError::InvalidConstruction(
            "use Displace\\Infer\\Model::load() to construct a Model".into(),
        )
        .into())
    }

    /// Load a GGUF model from disk.
    ///
    /// Recognised `$options` keys (kept as an array because load-time tuning
    /// is rare and noisy — chat/raw use named arguments instead):
    /// - `n_gpu_layers` (int, default 0)
    /// - `use_mmap` (bool, default true)
    /// - `use_mlock` (bool, default false)
    /// - `embedding` (bool, default false) — when `true`, `embed()` is
    ///   permitted on this handle. `chat()` and `raw()` are unaffected:
    ///   they build their own per-call context for generation regardless
    ///   of this flag. The flag exists to make the embedding intent
    ///   explicit at load time so a missing `pooling` option can be
    ///   validated up front instead of at the first `embed()` call.
    /// - `pooling` (string, default `"unspecified"`) — only consulted when
    ///   `embedding: true`. One of `"unspecified"` (trust GGUF metadata,
    ///   the default), `"none"`, `"mean"`, `"cls"`, `"last"`, `"rank"`.
    pub fn load(path: String, options: Option<&ZendHashTable>) -> PhpResult<Self> {
        let n_gpu_layers = get_uint(options, "n_gpu_layers")?.unwrap_or(0);
        let use_mmap = get_bool(options, "use_mmap")?.unwrap_or(true);
        let use_mlock = get_bool(options, "use_mlock")?.unwrap_or(false);
        let embedding_mode = get_bool(options, "embedding")?.unwrap_or(false);
        let pooling_type = match get_string(options, "pooling")?.as_deref() {
            None | Some("unspecified") => LlamaPoolingType::Unspecified,
            Some("none") => LlamaPoolingType::None,
            Some("mean") => LlamaPoolingType::Mean,
            Some("cls") => LlamaPoolingType::Cls,
            Some("last") => LlamaPoolingType::Last,
            Some("rank") => LlamaPoolingType::Rank,
            Some(other) => {
                return Err(InferError::InvalidOption {
                    name: "pooling".into(),
                    reason: format!(
                        "expected one of unspecified/none/mean/cls/last/rank, got {other:?}"
                    ),
                }
                .into());
            }
        };

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

        Ok(Self {
            inner: Some(model),
            embedding_mode,
            pooling_type,
        })
    }

    /// Run a chat completion against the loaded model.
    ///
    /// The `Prompt`'s messages are rendered through the model's *embedded*
    /// chat template (Qwen3, Llama 3, etc. ship the right Jinja template
    /// inside the GGUF) — callers never write `<|im_start|>` tokens by
    /// hand. The result is wrapped in a `Response` whose `answer()` getter
    /// returns the reply with any `<think>...</think>` blocks stripped, and
    /// whose `reasoning()` getter exposes what was stripped.
    ///
    /// Camel-cased parameter idents are intentional: PHP named-arguments use
    /// the parameter name verbatim and the PSR-12 convention is camelCase.
    ///
    /// Recognised `$options` keys (constraint tuning is rare enough to stay
    /// out of the named-argument surface):
    /// - `grammar` (string) — a GBNF grammar; sampling is constrained so
    ///   the output always matches it.
    /// - `schema` (array|string) — a JSON Schema (PHP array or JSON text),
    ///   compiled to GBNF internally. The supported subset is documented
    ///   on the website; unsupported keywords throw rather than silently
    ///   under-constraining. Mutually exclusive with `grammar`.
    #[php(defaults(maxTokens = 128, nCtx = 2048, temperature = 0.0, seed = 1234))]
    pub fn chat(
        &self,
        prompt: &Prompt,
        maxTokens: u32,
        nCtx: u32,
        temperature: f32,
        seed: u32,
        options: Option<&ZendHashTable>,
    ) -> PhpResult<Response> {
        let model = self.inner.as_ref().ok_or(InferError::Closed)?;

        // Build the llama.cpp message list. `LlamaChatMessage::new` rejects
        // null bytes in either field; surface that as a clear `Inference`
        // error rather than a generic FFI failure.
        let llama_messages: Vec<LlamaChatMessage> = prompt
            .messages_slice()
            .iter()
            .map(|m| LlamaChatMessage::new(m.role_owned(), m.content_owned()))
            .collect::<Result<_, _>>()
            .map_err(|e| {
                InferError::Inference(format!("chat message contains a null byte: {e}"))
            })?;

        // Render through the model's embedded chat template. `None` asks
        // llama-cpp-2 to read the template baked into the GGUF; any model
        // without one (rare for modern instruct GGUFs) surfaces as a clear
        // error rather than the engine silently picking ChatML.
        let template = model.chat_template(None).map_err(|e| {
            InferError::Inference(format!(
                "model has no embedded chat template — use Model::raw() for this model: {e}"
            ))
        })?;
        let rendered = model
            .apply_chat_template(&template, &llama_messages, /* add_assistant = */ true)
            .map_err(|e| InferError::Inference(format!("apply_chat_template failed: {e}")))?;

        // The chat template handles BOS itself, so we explicitly disable
        // our own BOS injection here.
        let result = run_completion(
            model,
            &rendered,
            RunOpts {
                max_tokens: maxTokens,
                n_ctx: nCtx,
                temperature,
                seed,
                add_bos: false,
                grammar: get_grammar(options)?,
            },
        )?;

        Ok(Response::new(
            result.text,
            result.finish_reason,
            result.tokens_generated,
        ))
    }

    /// Run a raw text completion. Escape hatch for callers who want full
    /// control over the prompt string (custom templates, base models, ...).
    /// Returns the generated text as a plain string — no reasoning split,
    /// no `Response` wrapper. If you want any of that, use `chat()`.
    ///
    /// `$options` accepts the same `grammar` / `schema` keys as `chat()`.
    // The parameter list *is* the public PHP API (each one is a PHP named
    // argument), so it can't be bundled into an options struct without
    // changing the userland surface.
    #[allow(clippy::too_many_arguments)]
    #[php(defaults(
        maxTokens = 128,
        nCtx = 2048,
        temperature = 0.0,
        seed = 1234,
        addBos = true
    ))]
    pub fn raw(
        &self,
        prompt: String,
        maxTokens: u32,
        nCtx: u32,
        temperature: f32,
        seed: u32,
        addBos: bool,
        options: Option<&ZendHashTable>,
    ) -> PhpResult<String> {
        let model = self.inner.as_ref().ok_or(InferError::Closed)?;
        let result = run_completion(
            model,
            &prompt,
            RunOpts {
                max_tokens: maxTokens,
                n_ctx: nCtx,
                temperature,
                seed,
                add_bos: addBos,
                grammar: get_grammar(options)?,
            },
        )?;
        Ok(result.text)
    }

    /// Generate a vector embedding for a single text.
    ///
    /// Requires the model to have been loaded with `embedding: true` —
    /// embedding mode flips a flag on the underlying context and is
    /// incompatible with generation in the same handle. Calling `embed()`
    /// on a generation-only handle throws `InferenceException` with a
    /// clear "load with embedding: true" message.
    ///
    /// Pooling defaults to whatever the GGUF metadata declares — purpose-
    /// built embedding GGUFs (BGE, E5, GTE, Qwen3-Embedding, ...) all
    /// embed their pooling strategy, so the default is the right answer
    /// for the overwhelming majority of cases. Override at load time with
    /// `['pooling' => 'mean' | 'cls' | 'last' | ...]` if a model ships
    /// without the metadata or you want to experiment.
    pub fn embed(&self, text: String) -> PhpResult<Embedding> {
        let model = self.inner.as_ref().ok_or(InferError::Closed)?;
        if !self.embedding_mode {
            return Err(InferError::Inference(
                "Model::embed() requires loading with ['embedding' => true]".into(),
            )
            .into());
        }
        let vector = run_embedding(model, &text, self.pooling_type)?;
        Ok(Embedding::from_vec(vector))
    }

    /// Release the underlying model weights. Idempotent; calling `close()`
    /// on an already-closed model is a no-op.
    pub fn close(&mut self) {
        // Dropping the inner `LlamaModel` releases its allocation. The
        // `LlamaBackend` itself stays alive — it's process-global.
        self.inner = None;
    }
}

// --- Inference core ---------------------------------------------------------

struct RunOpts {
    max_tokens: u32,
    n_ctx: u32,
    temperature: f32,
    seed: u32,
    add_bos: bool,
    /// GBNF grammar constraining the sampler, already compiled from a JSON
    /// Schema when the caller used the `schema` option.
    grammar: Option<String>,
}

/// What `run_completion` returns. Carries enough metadata for `Response` to
/// answer `finishReason()` / `tokensGenerated()` without re-deriving them.
struct CompletionResult {
    text: String,
    finish_reason: FinishReason,
    tokens_generated: u32,
}

fn run_completion(
    model: &LlamaModel,
    prompt: &str,
    opts: RunOpts,
) -> Result<CompletionResult, InferError> {
    let backend = backend()?;

    let n_ctx = NonZeroU32::new(opts.n_ctx).ok_or_else(|| InferError::InvalidOption {
        name: "n_ctx".into(),
        reason: "must be greater than zero".into(),
    })?;

    let ctx_params = LlamaContextParams::default().with_n_ctx(Some(n_ctx));
    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| InferError::Inference(format!("context creation failed: {e}")))?;

    let add_bos = if opts.add_bos {
        AddBos::Always
    } else {
        AddBos::Never
    };
    let prompt_tokens = model
        .str_to_token(prompt, add_bos)
        .map_err(|e| InferError::Inference(format!("tokenization failed: {e}")))?;

    if prompt_tokens.is_empty() {
        return Ok(CompletionResult {
            text: String::new(),
            finish_reason: FinishReason::Stop,
            tokens_generated: 0,
        });
    }

    let prompt_len = i32::try_from(prompt_tokens.len())
        .map_err(|_| InferError::Inference("prompt token count overflows i32".into()))?;
    if (prompt_len as u32) >= opts.n_ctx {
        return Err(InferError::Inference(format!(
            "prompt is {prompt_len} tokens but n_ctx is only {}",
            opts.n_ctx
        )));
    }

    // Submit the prompt as a single batch, asking for logits on the last
    // token only — that's the position from which we'll sample the first
    // generated token.
    let mut batch = LlamaBatch::new(BATCH_CAPACITY, 1);
    let last_prompt_index = prompt_len - 1;
    for (i, token) in prompt_tokens.into_iter().enumerate() {
        let i = i as i32;
        let is_last = i == last_prompt_index;
        batch
            .add(token, i, &[0], is_last)
            .map_err(|e| InferError::Inference(format!("batch add (prompt) failed: {e}")))?;
    }
    ctx.decode(&mut batch)
        .map_err(|e| InferError::Inference(format!("prompt decode failed: {e}")))?;

    // Sampling chain: greedy when temperature == 0, otherwise temperature +
    // distribution sampling seeded for reproducibility. A grammar sampler,
    // when present, goes *first* in the chain: it masks every token that
    // would violate the grammar before the actual sampling step picks from
    // what's left. Once the grammar's root rule is fully matched, only
    // end-of-generation tokens stay legal, so constrained runs finish with
    // `FinishReason::Eos` like any other completion.
    let grammar_sampler = opts
        .grammar
        .as_deref()
        .map(|g| {
            LlamaSampler::grammar(model, g, "root").map_err(|e| InferError::InvalidOption {
                name: "grammar".into(),
                reason: format!("llama.cpp rejected the GBNF grammar: {e}"),
            })
        })
        .transpose()?;
    let mut sampler = match (grammar_sampler, opts.temperature <= 0.0) {
        (Some(g), true) => LlamaSampler::chain_simple([g, LlamaSampler::greedy()]),
        (Some(g), false) => LlamaSampler::chain_simple([
            g,
            LlamaSampler::temp(opts.temperature),
            LlamaSampler::dist(opts.seed),
        ]),
        (None, true) => LlamaSampler::chain_simple([LlamaSampler::greedy()]),
        (None, false) => LlamaSampler::chain_simple([
            LlamaSampler::temp(opts.temperature),
            LlamaSampler::dist(opts.seed),
        ]),
    };

    let mut out_bytes: Vec<u8> = Vec::new();
    let mut n_cur = batch.n_tokens();
    let mut n_decoded: u32 = 0;
    let budget = i32::try_from(opts.max_tokens).unwrap_or(i32::MAX);

    let finish_reason = loop {
        if n_decoded >= opts.max_tokens || n_cur >= budget.saturating_add(prompt_len) {
            break FinishReason::Length;
        }

        // `sample()` wraps `llama_sampler_sample`, which already *accepts*
        // the chosen token into every sampler in the chain — do not call
        // `accept()` again here. A second accept is invisible with the
        // stateless greedy/temp/dist samplers but advances a stateful
        // sampler (grammar!) twice per token, desyncing it from the actual
        // output until llama.cpp aborts on `GGML_ASSERT(!stacks.empty())`.
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);

        if model.is_eog_token(token) {
            break FinishReason::Eos;
        }

        let piece = model
            .token_to_piece_bytes(token, 32, false, None)
            .map_err(|e| InferError::Inference(format!("detokenize failed: {e}")))?;
        out_bytes.extend_from_slice(&piece);

        batch.clear();
        batch
            .add(token, n_cur, &[0], true)
            .map_err(|e| InferError::Inference(format!("batch add (gen) failed: {e}")))?;
        n_cur += 1;
        n_decoded += 1;

        ctx.decode(&mut batch)
            .map_err(|e| InferError::Inference(format!("gen decode failed: {e}")))?;
    };

    // llama.cpp emits UTF-8 byte sequences that can straddle token
    // boundaries. Decode once at the end with replacement so we never
    // return invalid UTF-8 to PHP.
    Ok(CompletionResult {
        text: String::from_utf8_lossy(&out_bytes).into_owned(),
        finish_reason,
        tokens_generated: n_decoded,
    })
}

// --- Embedding core ---------------------------------------------------------

const EMBED_DEFAULT_N_CTX: u32 = 2048;
const EMBED_BATCH_CAPACITY: usize = 512;

/// Generate a single embedding vector for `text` using the given pooling.
///
/// We build a fresh context with `with_embeddings(true)` and the requested
/// pooling type, tokenize the input with the model's preferred BOS handling,
/// submit one batch, then pull the pooled vector via
/// `embeddings_seq_ith(0)`. The context is dropped at function exit — same
/// "no shared state between calls" model as `run_completion`.
fn run_embedding(
    model: &LlamaModel,
    text: &str,
    pooling: LlamaPoolingType,
) -> Result<Vec<f32>, InferError> {
    let backend = backend()?;

    let n_ctx_nz = NonZeroU32::new(EMBED_DEFAULT_N_CTX).expect("constant > 0");
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(n_ctx_nz))
        .with_embeddings(true)
        .with_pooling_type(pooling);
    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| InferError::Inference(format!("embedding context creation failed: {e}")))?;

    let tokens = model
        .str_to_token(text, AddBos::Always)
        .map_err(|e| InferError::Inference(format!("tokenization failed: {e}")))?;

    if tokens.is_empty() {
        return Err(InferError::Inference(
            "cannot embed empty text — tokenizer produced no tokens".into(),
        ));
    }
    let token_count = i32::try_from(tokens.len())
        .map_err(|_| InferError::Inference("input token count overflows i32".into()))?;
    if (token_count as u32) > EMBED_DEFAULT_N_CTX {
        return Err(InferError::Inference(format!(
            "input is {token_count} tokens but the embedding context is {EMBED_DEFAULT_N_CTX}"
        )));
    }

    let mut batch = LlamaBatch::new(EMBED_BATCH_CAPACITY, 1);
    let last = token_count - 1;
    for (i, token) in tokens.into_iter().enumerate() {
        let i = i as i32;
        // With `Last`/`Cls` pooling llama.cpp wants logits=true on the
        // appropriate position; with `Mean` it reads all hidden states.
        // Asking for logits on every token is harmless and works
        // uniformly across pooling strategies.
        let _ = last;
        batch
            .add(token, i, &[0], true)
            .map_err(|e| InferError::Inference(format!("batch add failed: {e}")))?;
    }
    ctx.decode(&mut batch)
        .map_err(|e| InferError::Inference(format!("embedding decode failed: {e}")))?;

    // `embeddings_seq_ith(0)` returns the pooled vector for sequence 0.
    // For `LlamaPoolingType::None`, that would be per-token instead and
    // require a different read path; we don't expose `None` as a sensible
    // default because it doesn't yield a single vector. Callers who pick
    // `pooling: 'none'` and then call `embed()` will see llama.cpp's
    // error here, which is the right surface.
    let slice = ctx
        .embeddings_seq_ith(0)
        .map_err(|e| InferError::Inference(format!("embedding read failed: {e}")))?;
    Ok(slice.to_vec())
}

// --- Option parsing helpers -------------------------------------------------
//
// Each helper takes the optional `$options` array passed from PHP to
// `Model::load`. A missing array (no second argument) and a missing key
// both yield `Ok(None)`. A key that *is* present but of the wrong type is
// a hard error — callers see an `InferException` with a useful message
// rather than the silent fallback to a default that a
// `from_zval(...).unwrap_or(default)` would give.

/// Resolve the `grammar` / `schema` keys of a `chat()` / `raw()` options
/// array into a ready-to-use GBNF string. The two keys are mutually
/// exclusive: `grammar` is handed to llama.cpp verbatim, `schema` (a JSON
/// Schema as a PHP array or JSON text) is compiled via
/// [`json_schema_to_gbnf`].
fn get_grammar(opts: Option<&ZendHashTable>) -> Result<Option<String>, InferError> {
    let grammar = get_string(opts, "grammar")?;
    let schema_zv = opts.and_then(|o| o.get("schema"));

    if grammar.is_some() && schema_zv.is_some() {
        return Err(InferError::InvalidOption {
            name: "schema".into(),
            reason: "'grammar' and 'schema' are mutually exclusive — pass one".into(),
        });
    }
    if let Some(g) = &grammar {
        if g.trim().is_empty() {
            return Err(InferError::InvalidOption {
                name: "grammar".into(),
                reason: "grammar string is empty".into(),
            });
        }
    }

    let Some(zv) = schema_zv else {
        return Ok(grammar);
    };

    let schema: serde_json::Value = if let Some(json_text) = zv.string() {
        serde_json::from_str(&json_text).map_err(|e| InferError::InvalidOption {
            name: "schema".into(),
            reason: format!("schema string is not valid JSON: {e}"),
        })?
    } else if zv.is_array() {
        zval_to_json(zv)?
    } else {
        return Err(InferError::InvalidOption {
            name: "schema".into(),
            reason: "expected a JSON Schema as an array or a JSON string".into(),
        });
    };

    Ok(Some(json_schema_to_gbnf(&schema)?))
}

/// Recursively convert a PHP value into `serde_json::Value` — the bridge
/// that lets callers write `['schema' => ['type' => 'object', ...]]` with
/// plain PHP arrays. Sequential arrays become JSON arrays, everything else
/// becomes a JSON object, mirroring `json_encode()`.
fn zval_to_json(zv: &ext_php_rs::types::Zval) -> Result<serde_json::Value, InferError> {
    use serde_json::Value;

    if zv.is_null() {
        return Ok(Value::Null);
    }
    if let Some(b) = zv.bool() {
        return Ok(Value::Bool(b));
    }
    if let Some(n) = zv.long() {
        return Ok(Value::Number(n.into()));
    }
    if let Some(f) = zv.double() {
        return serde_json::Number::from_f64(f)
            .map(Value::Number)
            .ok_or_else(|| InferError::InvalidOption {
                name: "schema".into(),
                reason: "schema contains a non-finite float (NaN/Inf)".into(),
            });
    }
    if let Some(s) = zv.string() {
        return Ok(Value::String(s));
    }
    if let Some(table) = zv.array() {
        if table.has_sequential_keys() {
            let mut items = Vec::with_capacity(table.len());
            for value in table.values() {
                items.push(zval_to_json(value)?);
            }
            return Ok(Value::Array(items));
        }
        let mut map = serde_json::Map::with_capacity(table.len());
        for (key, value) in table.iter() {
            let key: String = key.try_into().map_err(|_| InferError::InvalidOption {
                name: "schema".into(),
                reason: "schema array key is not representable as a string".into(),
            })?;
            map.insert(key, zval_to_json(value)?);
        }
        return Ok(Value::Object(map));
    }

    Err(InferError::InvalidOption {
        name: "schema".into(),
        reason: format!(
            "schema contains a value of unsupported type {:?}",
            zv.get_type()
        ),
    })
}

fn get_uint(opts: Option<&ZendHashTable>, key: &str) -> Result<Option<u32>, InferError> {
    let Some(zv) = opts.and_then(|o| o.get(key)) else {
        return Ok(None);
    };
    let n = i64::from_zval(zv).ok_or_else(|| InferError::InvalidOption {
        name: key.into(),
        reason: "expected integer".into(),
    })?;
    if n < 0 {
        return Err(InferError::InvalidOption {
            name: key.into(),
            reason: "must be non-negative".into(),
        });
    }
    u32::try_from(n)
        .map(Some)
        .map_err(|_| InferError::InvalidOption {
            name: key.into(),
            reason: "exceeds u32 range".into(),
        })
}

fn get_bool(opts: Option<&ZendHashTable>, key: &str) -> Result<Option<bool>, InferError> {
    let Some(zv) = opts.and_then(|o| o.get(key)) else {
        return Ok(None);
    };
    bool::from_zval(zv)
        .map(Some)
        .ok_or_else(|| InferError::InvalidOption {
            name: key.into(),
            reason: "expected bool".into(),
        })
}

fn get_string(opts: Option<&ZendHashTable>, key: &str) -> Result<Option<String>, InferError> {
    let Some(zv) = opts.and_then(|o| o.get(key)) else {
        return Ok(None);
    };
    String::from_zval(zv)
        .map(Some)
        .ok_or_else(|| InferError::InvalidOption {
            name: key.into(),
            reason: "expected string".into(),
        })
}
