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
     *                                      - `max_tokens`  (int,   default 128)
     *                                      - `n_ctx`       (int,   default 2048)
     *                                      - `temperature` (float, default 0.0 — greedy)
     *                                      - `seed`        (int,   default 1234)
     *                                      - `add_bos`     (bool,  default true)
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
