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
     * Run a synchronous completion against the loaded model and return the
     * generated text.
     *
     * @param string               $prompt  Input prompt; tokenized with BOS by default.
     * @param array<string, mixed> $options Recognised keys:
     *                                      - `max_tokens`     (int,   default 128)
     *                                      - `n_ctx`          (int,   default 2048)
     *                                      - `temperature`    (float, default 0.0 — greedy)
     *                                      - `seed`           (int,   default 1234)
     *                                      - `add_bos`        (bool,  default true)
     *                                      - `strip_thinking` (bool,  default false)
     *                                        — when true, remove `<think>...</think>`
     *                                        blocks from the returned text. Useful
     *                                        with reasoning models (Qwen3, R1, …)
     *                                        that wrap their internal monologue in
     *                                        those tags when prompted through a
     *                                        chat template.
     *
     * @throws \Displace\Infer\InferenceException If decoding or sampling fails,
     *                                            or if the model has been closed.
     */
    public function complete(string $prompt, array $options = []): string {}

    /**
     * Release the underlying model weights. Idempotent.
     */
    public function close(): void {}
}
