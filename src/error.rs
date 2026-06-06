//! Error types for `ext-infer`.
//!
//! On the Rust side, every fallible operation returns an [`InferError`]. On
//! the PHP side, those errors surface as a small hierarchy of exception
//! classes rooted at `Displace\Infer\InferException`, which itself extends
//! the built-in `\RuntimeException`.
//!
//! ```text
//! \RuntimeException
//!   └── Displace\Infer\InferException
//!         ├── Displace\Infer\ModelLoadException
//!         └── Displace\Infer\InferenceException
//! ```

use ext_php_rs::exception::PhpException;
use ext_php_rs::ffi::zend_class_entry;
use ext_php_rs::prelude::*;
use ext_php_rs::zend::ClassEntry;
use thiserror::Error;

/// Internal error type for fallible operations inside the extension.
///
/// Variants map 1:1 to the PHP-visible exception subclasses defined below.
/// New variants should be added alongside a corresponding `#[php_class]` so
/// that PHP callers can catch them precisely.
#[derive(Debug, Error)]
pub enum InferError {
    /// Failed to load a GGUF model from disk. Wraps any I/O, format, or
    /// backend-initialization failure that occurs during `Model::load()`.
    #[error("failed to load model: {0}")]
    ModelLoad(String),

    /// Inference failed mid-completion. Wraps tokenization, decode, or
    /// sampling errors that occur during `Model::complete()`.
    #[error("inference failed: {0}")]
    Inference(String),

    /// The model handle has already been closed and cannot be used again.
    #[error("model has been closed")]
    Closed,

    /// A caller-supplied option was malformed (wrong type, out of range).
    #[error("invalid option {name}: {reason}")]
    InvalidOption {
        /// The option key as supplied by the PHP caller.
        name: String,
        /// Human-readable explanation of what was wrong.
        reason: String,
    },

    /// Direct `new ClassName()` is not supported. Wraps a hint pointing the
    /// caller at the right factory (`Model::load()`, `Prompt::user()`, ...).
    #[error("{0}")]
    InvalidConstruction(String),
}

impl From<InferError> for PhpException {
    fn from(err: InferError) -> Self {
        let message = err.to_string();
        match err {
            InferError::ModelLoad(_) => PhpException::from_class::<ModelLoadException>(message),
            InferError::Inference(_) | InferError::Closed => {
                PhpException::from_class::<InferenceException>(message)
            }
            InferError::InvalidOption { .. } | InferError::InvalidConstruction(_) => {
                PhpException::from_class::<InferException>(message)
            }
        }
    }
}

/// Base exception for all `ext-infer` failures. Extends `\RuntimeException`
/// so existing `catch (\RuntimeException $e)` clauses continue to work.
#[php_class]
#[php(name = "Displace\\Infer\\InferException")]
#[php(extends(ce = runtime_exception_ce, stub = "\\RuntimeException"))]
#[derive(Default)]
pub struct InferException;

/// Thrown when a model file cannot be loaded — missing path, unreadable
/// file, unsupported quantization, or backend initialization failure.
#[php_class]
#[php(name = "Displace\\Infer\\ModelLoadException")]
#[php(extends(InferException))]
#[derive(Default)]
pub struct ModelLoadException;

/// Thrown when inference fails after a model has been successfully loaded —
/// tokenization, decode, sampling, or use-after-close errors.
#[php_class]
#[php(name = "Displace\\Infer\\InferenceException")]
#[php(extends(InferException))]
#[derive(Default)]
pub struct InferenceException;

// `\RuntimeException` is defined by SPL, which exposes its `zend_class_entry *`
// as a `PHPAPI` global — same convention as the engine's `zend_ce_*` globals
// that `ext_php_rs::zend::ce::*` wraps. SPL is a built-in module that is
// always loaded before user extensions, so by the time our MINIT runs (and
// `runtime_exception_ce()` is called for the `extends` linkage) this pointer
// is non-null.
//
// The alternative — `ClassEntry::try_find("RuntimeException")` — goes through
// `EG(class_table)`, which is not yet initialized during MINIT and would
// return `None`. Linking against the global directly avoids that ordering
// hazard entirely.
#[allow(non_upper_case_globals)]
unsafe extern "C" {
    static spl_ce_RuntimeException: *mut zend_class_entry;
}

/// Class-entry accessor for PHP's SPL `\RuntimeException`, used by the
/// `extends(ce = ...)` linkage on [`InferException`].
fn runtime_exception_ce() -> &'static ClassEntry {
    // SAFETY: `spl_ce_RuntimeException` is a stable PHPAPI symbol exported by
    // any SAPI we support. It is written once during SPL's MINIT (well before
    // ours) and never reassigned, so reading it as a shared `&'static` is
    // sound. A null pointer here would mean the host PHP is not SPL-enabled,
    // which is unsupported.
    unsafe { spl_ce_RuntimeException.as_ref() }
        .expect("SPL \\RuntimeException is required (host PHP missing the SPL extension?)")
}
