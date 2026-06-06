//! Fluent, immutable chat-prompt construction.
//!
//! The shape mirrors `DateTimeImmutable`: factory constructors for an initial
//! state, `with*` methods that return a new instance with one more message
//! appended. The original is never mutated, so a `Prompt` is safe to share or
//! cache across calls.
//!
//! ```php
//! $p = Displace\Infer\Prompt::system('You are helpful.')
//!     ->withUser('What is 2+2?');
//! ```
//!
//! Role strings (`'system'`, `'user'`, `'assistant'`) are the contract that
//! llama.cpp's `apply_chat_template` consumes — we keep them as plain strings
//! on the `Message` side so PHP code can compare them with literals rather
//! than importing an extra enum class. Method-name discipline on the input
//! side (`withSystem`/`withUser`/`withAssistant`) means typos can't slip in.

use ext_php_rs::prelude::*;

use crate::error::InferError;

const ROLE_SYSTEM: &str = "system";
const ROLE_USER: &str = "user";
const ROLE_ASSISTANT: &str = "assistant";

/// A single message in a chat prompt.
///
/// Read-only. Instances are produced by `Prompt`'s builder methods; PHP code
/// can inspect them via `messages()` but cannot construct them directly —
/// allowing arbitrary role strings would only push validation errors to
/// `apply_chat_template` time with worse messages.
#[php_class]
#[php(name = "Displace\\Infer\\Message")]
#[derive(Default, Clone)]
pub struct Message {
    role: String,
    content: String,
}

#[php_impl]
impl Message {
    /// Refuse direct construction. `Prompt::user()` / `withUser()` etc. are
    /// the only legal entry points.
    pub fn __construct() -> PhpResult<Self> {
        Err(InferError::InvalidConstruction(
            "Displace\\Infer\\Message is produced by Prompt; do not instantiate directly".into(),
        )
        .into())
    }

    /// One of `'system'`, `'user'`, or `'assistant'`.
    pub fn role(&self) -> String {
        self.role.clone()
    }

    /// The verbatim message body.
    pub fn content(&self) -> String {
        self.content.clone()
    }
}

impl Message {
    /// Rust-side constructor used by `Prompt`.
    pub(crate) fn new(role: &str, content: String) -> Self {
        Self {
            role: role.to_string(),
            content,
        }
    }

    /// Cloned role string, used by `Model::chat()` to build the
    /// `LlamaChatMessage` list it hands to `apply_chat_template`.
    pub(crate) fn role_owned(&self) -> String {
        self.role.clone()
    }

    /// Cloned content string, paired with `role_owned`.
    pub(crate) fn content_owned(&self) -> String {
        self.content.clone()
    }
}

/// Ordered, immutable list of chat messages.
///
/// Construction is two-stage: pick a factory (`system()` or `user()`) for the
/// first message, then chain `with*` calls for subsequent ones. Each `with*`
/// returns a fresh `Prompt` with the new message appended — the receiver is
/// never modified.
#[php_class]
#[php(name = "Displace\\Infer\\Prompt")]
#[derive(Default, Clone)]
pub struct Prompt {
    messages: Vec<Message>,
}

#[php_impl]
impl Prompt {
    /// Refuse direct construction. Use `Prompt::system()` or `Prompt::user()`
    /// to start a prompt — the two-stage factory + `with*` shape mirrors
    /// `DateTimeImmutable` and keeps the API discoverable.
    pub fn __construct() -> PhpResult<Self> {
        Err(InferError::InvalidConstruction(
            "use Displace\\Infer\\Prompt::system() or Prompt::user() to start a prompt".into(),
        )
        .into())
    }

    /// Start a prompt with a system message.
    pub fn system(content: String) -> Self {
        Self {
            messages: vec![Message::new(ROLE_SYSTEM, content)],
        }
    }

    /// Start a prompt with a user message — the common shape when there's no
    /// system instruction to set.
    pub fn user(content: String) -> Self {
        Self {
            messages: vec![Message::new(ROLE_USER, content)],
        }
    }

    /// Return a new `Prompt` with a system message appended. The receiver is
    /// not modified. Multiple system messages are permitted; whether the
    /// underlying model accepts them is a chat-template decision, not ours.
    pub fn with_system(&self, content: String) -> Self {
        self.appended(ROLE_SYSTEM, content)
    }

    /// Return a new `Prompt` with a user message appended.
    pub fn with_user(&self, content: String) -> Self {
        self.appended(ROLE_USER, content)
    }

    /// Return a new `Prompt` with an assistant message appended. Useful for
    /// multi-turn conversations where the caller is replaying history.
    pub fn with_assistant(&self, content: String) -> Self {
        self.appended(ROLE_ASSISTANT, content)
    }

    /// The messages in order, as an array of `Message` instances.
    pub fn messages(&self) -> Vec<Message> {
        self.messages.clone()
    }

    /// The role of the most-recently-appended message, or `null` if the
    /// prompt is empty (only reachable via the (forbidden) default
    /// constructor, but here for completeness).
    pub fn last_role(&self) -> Option<String> {
        self.messages.last().map(|m| m.role.clone())
    }

    /// Number of messages in the prompt.
    pub fn count(&self) -> usize {
        self.messages.len()
    }

    /// `true` when there are no messages.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

impl Prompt {
    fn appended(&self, role: &str, content: String) -> Self {
        let mut messages = self.messages.clone();
        messages.push(Message::new(role, content));
        Self { messages }
    }

    /// Rust-side slice accessor used by `Model::chat()`.
    pub(crate) fn messages_slice(&self) -> &[Message] {
        &self.messages
    }
}
