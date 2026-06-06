//! `ext-infer` — PHP 8.3+ native, in-process LLM inference via llama.cpp.
//!
//! Public surface:
//!
//! - `Displace\Infer\Model`              — load + run completions / embeddings
//! - `Displace\Infer\Prompt`             — immutable chat-prompt builder
//! - `Displace\Infer\Message`            — single message in a `Prompt`
//! - `Displace\Infer\InferException`     — base exception (extends `\RuntimeException`)
//! - `Displace\Infer\ModelLoadException` — load-time failure
//! - `Displace\Infer\InferenceException` — runtime failure
//!
//! See the per-module docs for design notes.

#![deny(clippy::all)]

mod error;
mod model;
mod prompt;

use ext_php_rs::prelude::*;

// Re-export so `cargo php stubs` and module registration can see them by
// their crate-root paths.
pub use error::{InferException, InferenceException, ModelLoadException};
pub use model::Model;
pub use prompt::{Message, Prompt};

/// PHP module entry point.
///
/// The default module name is `CARGO_PKG_NAME` (`ext-infer`); we override
/// it to plain `infer` so userland calls `extension_loaded('infer')` —
/// matching PHP's convention of dropping the `ext-` prefix.
///
/// The order of `class::<T>()` calls is significant: child exceptions
/// reference their parent's `ClassEntry`, so the parent must be registered
/// first.
#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
        .name("infer")
        .class::<InferException>()
        .class::<ModelLoadException>()
        .class::<InferenceException>()
        .class::<Message>()
        .class::<Prompt>()
        .class::<Model>()
}
