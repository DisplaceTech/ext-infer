//! `Displace\Infer\Model` — the only PHP class exposed in Phase 1.
//!
//! Lifecycle:
//!
//! ```php
//! $model = \Displace\Infer\Model::load('/path/to/llama.gguf');
//! $text  = $model->complete('Once upon a time, ');
//! $model->close();
//! ```
//!
//! A `Model` owns a [`LlamaModel`] (the in-memory weights). Each
//! `complete()` call constructs a fresh [`LlamaContext`] from those weights,
//! runs a synchronous decode/sample loop, and drops the context. Phase 2
//! will introduce reusable session contexts; the public surface here is
//! intentionally shaped so that can be added without a breaking change.

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use ext_php_rs::convert::FromZval;
use ext_php_rs::prelude::*;
use ext_php_rs::types::ZendHashTable;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

use crate::error::InferError;

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

// --- Phase 1 defaults -------------------------------------------------------

const DEFAULT_N_CTX: u32 = 2048;
const DEFAULT_MAX_TOKENS: u32 = 128;
const DEFAULT_TEMPERATURE: f32 = 0.0;
const DEFAULT_SEED: u32 = 1234;
const DEFAULT_ADD_BOS: bool = true;
const BATCH_CAPACITY: usize = 512;

// --- Model ------------------------------------------------------------------

/// PHP-visible handle to a loaded GGUF model.
///
/// Internally wraps an `Option<LlamaModel>` so that `close()` can release the
/// underlying weights deterministically rather than waiting for PHP's GC.
/// After `close()`, every other method throws `InferenceException`.
#[php_class]
#[php(name = "Displace\\Infer\\Model")]
#[derive(Default)]
pub struct Model {
    inner: Option<LlamaModel>,
}

#[php_impl]
impl Model {
    /// Direct construction is not supported — use `Model::load()`. This
    /// constructor exists only to give callers a clear error if they try
    /// `new Model()` out of habit.
    pub fn __construct() -> PhpResult<Self> {
        Err(
            InferError::ModelLoad("use Displace\\Infer\\Model::load() to construct a Model".into())
                .into(),
        )
    }

    /// Load a GGUF model from disk.
    ///
    /// Recognised `$options` keys:
    /// - `n_gpu_layers` (int, default 0)
    /// - `use_mmap` (bool, default true)
    /// - `use_mlock` (bool, default false)
    pub fn load(path: String, options: Option<&ZendHashTable>) -> PhpResult<Self> {
        let n_gpu_layers = get_uint(options, "n_gpu_layers")?.unwrap_or(0);
        let use_mmap = get_bool(options, "use_mmap")?.unwrap_or(true);
        let use_mlock = get_bool(options, "use_mlock")?.unwrap_or(false);

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

        Ok(Self { inner: Some(model) })
    }

    /// Run a synchronous completion against the loaded model.
    ///
    /// Recognised `$options` keys:
    /// - `max_tokens` (int, default 128)
    /// - `n_ctx` (int, default 2048)
    /// - `temperature` (float, default 0.0 — greedy)
    /// - `seed` (int, default 1234)
    /// - `add_bos` (bool, default true)
    pub fn complete(&self, prompt: String, options: Option<&ZendHashTable>) -> PhpResult<String> {
        let model = self.inner.as_ref().ok_or(InferError::Closed)?;

        let max_tokens = get_uint(options, "max_tokens")?.unwrap_or(DEFAULT_MAX_TOKENS);
        let n_ctx = get_uint(options, "n_ctx")?.unwrap_or(DEFAULT_N_CTX);
        let temperature = get_float(options, "temperature")?.unwrap_or(DEFAULT_TEMPERATURE);
        let seed = get_uint(options, "seed")?.unwrap_or(DEFAULT_SEED);
        let add_bos = get_bool(options, "add_bos")?.unwrap_or(DEFAULT_ADD_BOS);

        let text = run_completion(
            model,
            &prompt,
            RunOpts {
                max_tokens,
                n_ctx,
                temperature,
                seed,
                add_bos,
            },
        )?;
        Ok(text)
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
}

fn run_completion(model: &LlamaModel, prompt: &str, opts: RunOpts) -> Result<String, InferError> {
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
        return Ok(String::new());
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
    // distribution sampling seeded for reproducibility.
    let mut sampler = if opts.temperature <= 0.0 {
        LlamaSampler::chain_simple([LlamaSampler::greedy()])
    } else {
        LlamaSampler::chain_simple([
            LlamaSampler::temp(opts.temperature),
            LlamaSampler::dist(opts.seed),
        ])
    };

    let mut out_bytes: Vec<u8> = Vec::new();
    let mut n_cur = batch.n_tokens();
    let mut n_decoded: u32 = 0;
    let budget = i32::try_from(opts.max_tokens).unwrap_or(i32::MAX);

    while n_decoded < opts.max_tokens && n_cur < budget.saturating_add(prompt_len) {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);

        if model.is_eog_token(token) {
            break;
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
    }

    // llama.cpp emits UTF-8 byte sequences that can straddle token
    // boundaries. Decode once at the end with replacement so we never
    // return invalid UTF-8 to PHP.
    Ok(String::from_utf8_lossy(&out_bytes).into_owned())
}

// --- Option parsing helpers -------------------------------------------------
//
// Each helper takes the optional `$options` array passed from PHP. A missing
// array (no second argument) and a missing key both yield `Ok(None)`. A key
// that *is* present but of the wrong type is a hard error — callers see an
// `InferException` with a useful message rather than the silent fallback to
// a default that a `from_zval(...).unwrap_or(default)` would give.

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

fn get_float(opts: Option<&ZendHashTable>, key: &str) -> Result<Option<f32>, InferError> {
    let Some(zv) = opts.and_then(|o| o.get(key)) else {
        return Ok(None);
    };
    // Accept both PHP floats and PHP ints — `1` and `1.0` should both work.
    if let Some(f) = f64::from_zval(zv) {
        return Ok(Some(f as f32));
    }
    if let Some(i) = i64::from_zval(zv) {
        return Ok(Some(i as f32));
    }
    Err(InferError::InvalidOption {
        name: key.into(),
        reason: "expected float".into(),
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
