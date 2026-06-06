# ext-infer

`ext-infer` is a PHP 8.3+ extension that brings local LLM inference into the
PHP process itself, via [llama.cpp](https://github.com/ggerganov/llama.cpp).
It exists so that PHP-native semantic search, RAG pipelines, and CLI/worker
inference can run without shelling out to Python or hitting a remote API.
The extension is written in Rust on top of [`ext-php-rs`](https://github.com/davidcole1340/ext-php-rs)
and the [`llama-cpp-2`](https://crates.io/crates/llama-cpp-2) bindings.

> Status: **Phase 1 — pre-release.** The API is small and intentionally
> conservative; see the roadmap below.

## Hello, model

```php
<?php
use Displace\Infer\Model;

$model = Model::load('/models/llama-3.2-1b-instruct-q4_k_m.gguf');

$reply = $model->complete('The capital of France is', [
    'max_tokens'  => 32,
    'temperature' => 0.0,
]);

echo $reply, PHP_EOL;

$model->close();
```

## Install

### Prerequisites

- PHP 8.3 or 8.4, with `php-config` on `PATH`
- Rust toolchain (stable; the repo pins via `rust-toolchain.toml`)
- `cmake` and a C/C++ toolchain (llama.cpp builds during the cargo build)
- `cargo install cargo-php` once, for the `cargo php` subcommands

### Build & load into your local PHP

```sh
git clone https://github.com/displace/ext-infer
cd ext-infer
make release          # builds target/release/libinfer.{so,dylib}
make install          # cargo php install --release
php -m | grep infer   # confirm it loaded
```

### Apple Metal acceleration (opt-in)

The default build is CPU-only and portable. To enable Metal on
macOS-arm64:

```sh
make release FEATURES=metal
make install  FEATURES=metal
```

Metal will likely become the default for macOS in Phase 2 once we've
validated the build on a wider matrix.

## API surface (Phase 1)

```text
Displace\Infer\Model
    public static function load(string $path, array $options = []): self
    public function complete(string $prompt, array $options = []): string
    public function close(): void

Displace\Infer\InferException        extends \RuntimeException
Displace\Infer\ModelLoadException    extends Displace\Infer\InferException
Displace\Infer\InferenceException    extends Displace\Infer\InferException
```

### `Model::load()` options

| Key            | Type | Default | Notes                                  |
| -------------- | ---- | ------- | -------------------------------------- |
| `n_gpu_layers` | int  | `0`     | Layers offloaded to GPU; CPU when `0`. |
| `use_mmap`     | bool | `true`  | Memory-map the GGUF file.              |
| `use_mlock`    | bool | `false` | Lock weights in RAM.                   |

### `Model::complete()` options

| Key           | Type  | Default | Notes                                          |
| ------------- | ----- | ------- | ---------------------------------------------- |
| `max_tokens`  | int   | `128`   | Hard cap on generated tokens.                  |
| `n_ctx`       | int   | `2048`  | Context window for this call.                  |
| `temperature` | float | `0.0`   | `0.0` is greedy; anything `> 0` samples.       |
| `seed`        | int   | `1234`  | RNG seed used when `temperature > 0`.          |
| `add_bos`     | bool  | `true`  | Prepend the model's beginning-of-sequence tok. |

## Development

```sh
make build       # debug build
make clippy      # cargo clippy -- -D warnings
make fmt-check   # cargo fmt --check
make stubs       # regenerate stubs/infer.stubs.php from the registered classes
make test        # PHPT suite (needs php run-tests.php; see Makefile)
```

PHPT tests that require an actual model file are gated on the
`INFER_TEST_MODEL` env var:

```sh
INFER_TEST_MODEL=/models/llama-3.2-1b-instruct-q4_k_m.gguf make test
```

## Roadmap

**Phase 1 (this release)**

- Load GGUF from disk
- Single synchronous completion → string
- Typed exception hierarchy
- PHPT smoke tests + CI on PHP {8.3, 8.4} × {macOS-arm64, ubuntu-latest}

**Phase 2 (planned, not yet committed)**

- Streaming completions (PHP generators / callbacks)
- Embeddings (`Model::embed(string|array): array`)
- Chat templates and conversation state
- Reusable session/context objects (KV-cache reuse across calls)
- Tool calling
- Continuous batching for worker scenarios
- Metal-on-by-default for macOS-arm64

## License

MIT — see [LICENSE](LICENSE).
