<?php

// Stubs for ext-infer — IDE / static-analysis only, not loaded at runtime.
//
// Regenerate from the registered classes after building:
//
//     make stubs   # wraps `cargo php stubs --stubs stubs/infer.stubs.php`

namespace Displace\Infer;

class InferException extends \RuntimeException
{
}

class ModelLoadException extends \Displace\Infer\InferException
{
}

class InferenceException extends \Displace\Infer\InferException
{
}

/**
 * Single embedding vector produced by `Model::embed()`.
 *
 * Read-only. Carries the float vector plus a small amount of vector math
 * (dimensionality, L2 norm, unit-length normalization, cosine similarity).
 * Direct construction is refused — a vector built by PHP would lie about
 * which model produced it and what pooling strategy was applied.
 */
final class Embedding
{
    /** @throws \Displace\Infer\InferException Always. */
    public function __construct() {}

    /**
     * The embedding as a flat array of floats. Length matches `dimensions()`.
     *
     * @return list<float>
     */
    public function vector(): array {}

    /** Vector length — the loaded model's `n_embd`. */
    public function dimensions(): int {}

    /**
     * The embedding as a packed little-endian float32 binary string —
     * byte-identical to `pack('g*', ...$embedding->vector())`, the format
     * every Displace vector API speaks. The bytes are produced from the
     * Rust-held float32 vector, so coordinates are never inflated into
     * PHP values. Prefer this over `vector()` whenever the destination
     * wants packed bytes (`Displace\Vector` indexes, ...).
     */
    public function packed(): string {}

    /** L2 norm of the vector. */
    public function norm(): float {}

    /** Return a new Embedding scaled to unit length (zero-vectors are returned unchanged). */
    public function normalize(): self {}

    /**
     * Cosine similarity against another embedding.
     *
     * @throws \Displace\Infer\InferenceException If the two vectors have different dimensions.
     */
    public function cosineSimilarity(\Displace\Infer\Embedding $other): float {}
}

/**
 * Result of a `Model::chat()` call.
 *
 * Reasoning-model output (Qwen3, DeepSeek R1, ...) is split into the
 * `<think>...</think>` content (available via `reasoning()`) and the actual
 * answer that follows (available via `answer()`); the raw concatenation is
 * always accessible via `text()`.
 *
 * Read-only. Instances are produced by `Model::chat()`; direct construction
 * throws `InferException`.
 */
final class Response
{
    /** @throws \Displace\Infer\InferException Always. */
    public function __construct() {}

    /** Full model output, including any `<think>` block(s). Same as a raw completion. */
    public function text(): string {}

    /** Reasoning extracted from `<think>...</think>` block(s), or `null` if none was emitted. */
    public function reasoning(): ?string {}

    /** `text()` with `<think>...</think>` block(s) removed; the model's actual answer. */
    public function answer(): string {}

    /** `true` if the model emitted any `<think>...</think>` content. */
    public function hasReasoning(): bool {}

    /** Why generation stopped: `'eos'`, `'length'`, or `'stop'`. */
    public function finishReason(): string {}

    /** Number of tokens the model generated (prompt tokens are not counted). */
    public function tokensGenerated(): int {}
}

/**
 * A single message in a chat `Prompt`.
 *
 * Read-only. Instances are produced by the `Prompt` builder methods; direct
 * construction is refused (`new Message()` throws `InferException`).
 */
final class Message
{
    /** @throws \Displace\Infer\InferException Always. */
    public function __construct() {}

    /** One of `'system'`, `'user'`, or `'assistant'`. */
    public function role(): string {}

    /** The message body, verbatim. */
    public function content(): string {}
}

/**
 * Immutable, fluent chat-prompt builder.
 *
 * Start with `Prompt::system($s)` or `Prompt::user($s)`; chain `with*` calls
 * for additional turns. Every `with*` returns a new `Prompt` — the receiver
 * is never modified, in the style of `DateTimeImmutable::add()`.
 *
 * ```php
 * $p = Prompt::system('You are helpful.')
 *     ->withUser('What is 2+2?');
 * ```
 *
 * The resulting `Prompt` is fed to `Model::chat()`, which renders it through
 * the model's embedded chat template — callers never need to write
 * `<|im_start|>` tokens by hand.
 */
final class Prompt
{
    /** @throws \Displace\Infer\InferException Always — use `Prompt::system()` or `Prompt::user()`. */
    public function __construct() {}

    /** Start a prompt with a system message. */
    public static function system(string $content): self {}

    /** Start a prompt with a user message. */
    public static function user(string $content): self {}

    /** Return a new prompt with a system message appended. */
    public function withSystem(string $content): self {}

    /** Return a new prompt with a user message appended. */
    public function withUser(string $content): self {}

    /** Return a new prompt with an assistant message appended (useful for replaying history). */
    public function withAssistant(string $content): self {}

    /**
     * The messages in order.
     *
     * @return list<\Displace\Infer\Message>
     */
    public function messages(): array {}

    /** Role of the most recently appended message, or `null` if empty. */
    public function lastRole(): ?string {}

    /** Number of messages in the prompt. */
    public function count(): int {}

    /** `true` when there are no messages. */
    public function isEmpty(): bool {}
}

class Model
{
    /**
     * Load a GGUF model from disk.
     *
     * @param string               $path    Filesystem path to a `.gguf` file.
     * @param array<string, mixed> $options Recognised keys:
     *                                      - `n_gpu_layers` (int, default 0)
     *                                      - `use_mmap` (bool, default true)
     *                                      - `use_mlock` (bool, default false)
     *
     * @throws \Displace\Infer\ModelLoadException If the file cannot be read or parsed.
     */
    public static function load(string $path, array $options = []): self {}

    /**
     * Run a chat completion against the loaded model.
     *
     * The `Prompt` is rendered through the model's embedded chat template
     * (Qwen3, Llama 3, … all ship a Jinja template inside the GGUF), so
     * callers never need to write `<|im_start|>` tokens by hand.
     *
     * @param \Displace\Infer\Prompt $prompt
     *
     * @throws \Displace\Infer\InferenceException If the model has been closed,
     *                                            has no embedded chat template,
     *                                            or fails to decode the prompt.
     */
    public function chat(
        \Displace\Infer\Prompt $prompt,
        int $maxTokens = 128,
        int $nCtx = 2048,
        float $temperature = 0.0,
        int $seed = 1234,
    ): \Displace\Infer\Response {}

    /**
     * Run a raw text completion. Escape hatch for callers who want full
     * control over the prompt string — custom templates, base models, etc.
     * Returns the generated text as a plain string with no reasoning split.
     *
     * @throws \Displace\Infer\InferenceException If decoding or sampling fails,
     *                                            or if the model has been closed.
     */
    public function raw(
        string $prompt,
        int $maxTokens = 128,
        int $nCtx = 2048,
        float $temperature = 0.0,
        int $seed = 1234,
        bool $addBos = true,
    ): string {}

    /**
     * Generate an embedding vector for a single text.
     *
     * Requires the model to have been loaded with `['embedding' => true]`.
     * Pooling defaults to whatever the GGUF metadata declares, which is
     * correct for purpose-built embedding models (BGE, E5, GTE,
     * Qwen3-Embedding, …); override via `['pooling' => 'mean'|'cls'|'last'|…]`
     * on `Model::load()` if a model ships without the metadata.
     *
     * @throws \Displace\Infer\InferenceException If the model was not loaded with
     *                                            `embedding: true`, if the model
     *                                            has been closed, or if encoding
     *                                            fails.
     */
    public function embed(string $text): \Displace\Infer\Embedding {}

    /**
     * Release the underlying model weights. Idempotent.
     */
    public function close(): void {}
}
