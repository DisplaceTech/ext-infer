# ext-infer

`ext-infer` is a PHP 8.3+ extension that brings local LLM inference into
the PHP process itself, via [llama.cpp](https://github.com/ggerganov/llama.cpp).
It exists so that PHP-native semantic search, RAG pipelines, and CLI/worker
inference can run without shelling out to Python or hitting a remote API.

The extension is written in Rust on top of [`ext-php-rs`](https://github.com/davidcole1340/ext-php-rs)
and the [`llama-cpp-2`](https://crates.io/crates/llama-cpp-2) bindings.
The public PHP surface is fluent and role-aware — building a chat prompt
looks like `Prompt::system(...)->withUser(...)`, not a string of
`<|im_start|>` tokens.

> Status: **pre-1.0.** The class surface is stable; we'll cut `v0.1.0` once
> the release-binary pipeline is exercised end-to-end. See
> [`RELEASE.md`](RELEASE.md) for the cut-a-release flow.

## Hello, model

Grab a small GGUF and run the bundled example. Qwen3-0.6B-Q8 (~640 MB,
Apache-2.0) loads in under a second on Apple Silicon and is small
enough to keep around as a fixture:

```sh
mkdir -p models
curl -L -o models/Qwen3-0.6B-Q8_0.gguf \
    https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q8_0.gguf
```

Then either install the extension (`make install` or — once we ship
binaries — `pie install displace/ext-infer`) or point at the local
build with `-d extension=...` and run [`examples/hello-world.php`](examples/hello-world.php):

```sh
php -d extension=$PWD/target/debug/libinfer.dylib \
    examples/hello-world.php models/Qwen3-0.6B-Q8_0.gguf
# => 2 + 2 equals 4.
```

The minimal program:

```php
<?php
use Displace\Infer\Model;
use Displace\Infer\Prompt;

$model = Model::load('/path/to/llama.gguf');

$response = $model->chat(
    Prompt::system('You are a helpful assistant.')
        ->withUser('What is 2+2?'),
    maxTokens: 256,
    temperature: 0.0,
);

echo $response->answer(), PHP_EOL;  // "2 + 2 equals 4."

$model->close();
```

`$response->reasoning()` exposes whatever the model emitted inside
`<think>...</think>` blocks; `$response->text()` is the full
concatenation. For full control over the prompt string (custom
templates, base models without a chat template, etc.), use
`$model->raw('Once upon a time, ', maxTokens: 32)`.

llama.cpp's own stderr chatter (model layout, KV-cache sizing, graph
reservation) is silenced by default — it's not useful inside a PHP
request and tends to poison structured logs. Set `EXT_INFER_LOG=1` in
the environment if you want to see it.

## Embeddings

`Model::embed()` returns an `Embedding` with vector math built in:

```php
$model = Model::load('/path/to/embedding-model.gguf', ['embedding' => true]);

$a = $model->embed('The cat sat on the mat.')->normalize();
$b = $model->embed('A feline rested on the rug.')->normalize();

echo $a->cosineSimilarity($b), PHP_EOL;  // ~0.72 with Qwen3-Embedding-0.6B
echo $a->dimensions(), PHP_EOL;          // 1024
```

See [`examples/embedding.php`](examples/embedding.php) for the
pairwise-similarity flow, and [`examples/README.md`](examples/README.md)
for the recommended embedding model.

## Install

### Via PIE (preferred — once we publish v0.1.0)

```sh
# Install PIE first; see https://github.com/php/pie for current docs
curl -L --output pie.phar \
    https://github.com/php/pie/releases/latest/download/pie.phar
chmod +x pie.phar && sudo mv pie.phar /usr/local/bin/pie

pie install displace/ext-infer
php -m | grep infer
```

PIE fetches the appropriate pre-built binary for your `(php-minor,
arch, os, libc)` combo from the GitHub Release for the tag you're
installing. No local C/C++ toolchain required.

### From source

```sh
git clone https://github.com/DisplaceTech/ext-infer
cd ext-infer
make release          # builds target/release/libinfer.{so,dylib}
make install          # cargo php install --release
php -m | grep infer   # confirm it loaded
```

Prereqs for the source build:

- PHP 8.3 or 8.4, with `php-config` on `PATH`
- Rust toolchain (stable; the repo pins via `rust-toolchain.toml`)
- `cmake` and a C/C++ toolchain (llama.cpp builds during the cargo build)
- `cargo install cargo-php` once, for the `cargo php` subcommands

### Apple Metal acceleration (opt-in)

The default release build is CPU-only and portable. For Apple Silicon
GPU offload:

```sh
make release FEATURES=metal
make install  FEATURES=metal
```

We'll flip Metal on by default once the macos-14 GitHub runner's
hardware mix is exercised end-to-end.

## API surface

```text
Displace\Infer\
    Model::load(string $path, array $options = []): self

    Model::chat(Prompt $prompt,
                int $maxTokens = 128,
                int $nCtx = 2048,
                float $temperature = 0.0,
                int $seed = 1234): Response
    Model::raw(string $prompt,
               int $maxTokens = 128,
               int $nCtx = 2048,
               float $temperature = 0.0,
               int $seed = 1234,
               bool $addBos = true): string
    Model::embed(string $text): Embedding
    Model::close(): void

    Prompt::system(string $content): self
    Prompt::user(string $content): self
    Prompt::withSystem(string): self
    Prompt::withUser(string): self
    Prompt::withAssistant(string): self
    Prompt::messages(): list<Message>
    Prompt::lastRole(): ?string
    Prompt::count(): int
    Prompt::isEmpty(): bool

    Message::role(): string         // 'system' | 'user' | 'assistant'
    Message::content(): string

    Response::text(): string        // full output, <think>…</think> + answer
    Response::reasoning(): ?string  // <think>…</think> content, or null
    Response::answer(): string      // text() with reasoning stripped
    Response::hasReasoning(): bool
    Response::finishReason(): string  // 'eos' | 'length' | 'stop'
    Response::tokensGenerated(): int

    Embedding::vector(): list<float>
    Embedding::dimensions(): int
    Embedding::norm(): float
    Embedding::normalize(): self
    Embedding::cosineSimilarity(Embedding $other): float

    InferException        extends \RuntimeException
    ├── ModelLoadException
    └── InferenceException
```

`Prompt`, `Message`, `Response`, and `Embedding` all refuse direct
`new` — they're either built via static factories (`Prompt::system`,
`Prompt::user`) or returned from `Model` methods. `Prompt` is
immutable: every `with*` returns a new instance, in the style of
`DateTimeImmutable::add()`.

### `Model::load()` options

| Key            | Type   | Default         | Notes                                                                                                |
| -------------- | ------ | --------------- | ---------------------------------------------------------------------------------------------------- |
| `n_gpu_layers` | int    | `0`             | Layers offloaded to GPU; CPU when `0`.                                                               |
| `use_mmap`     | bool   | `true`          | Memory-map the GGUF file.                                                                            |
| `use_mlock`    | bool   | `false`         | Lock weights in RAM.                                                                                 |
| `embedding`    | bool   | `false`         | Enable `embed()` on this handle. Generation methods (`chat`/`raw`) still work on an embedding handle.|
| `pooling`      | string | `'unspecified'` | One of `unspecified`/`none`/`mean`/`cls`/`last`/`rank`. Default trusts GGUF metadata.                |

### Reasoning models

Reasoning-tuned models (Qwen3, DeepSeek R1, …) wrap their internal
monologue in `<think>...</think>` blocks when invoked through their
chat template. `Model::chat()` handles the template automatically;
the `Response` it returns splits the reasoning out for you:

```php
$r = $model->chat(Prompt::user('What is 2+2?'));

$r->text();         // "<think>Okay so 2+2…</think>\n\n2 + 2 equals 4."
$r->reasoning();    // "Okay so 2+2…"
$r->answer();       // "2 + 2 equals 4."
$r->hasReasoning(); // true
```

For non-reasoning models, `reasoning()` is `null` and `answer()` equals
`text()` byte-for-byte.

## Examples

See [`examples/`](examples/) for full-context walkthroughs:

- [`hello-world.php`](examples/hello-world.php) — one-shot chat completion.
- [`embedding.php`](examples/embedding.php) — pairwise semantic similarity.
- [`chat-interactive/`](examples/chat-interactive/) — Symfony Console
  multi-turn chat app. Demonstrates immutable `Prompt` accumulation
  across turns, reasoning-model handling, and graceful inference errors.

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
INFER_TEST_MODEL=$PWD/models/Qwen3-0.6B-Q8_0.gguf make test
```

## Releasing

See [`RELEASE.md`](RELEASE.md). TL;DR: bump `Cargo.toml`, push a `v*`
tag, the [release workflow](.github/workflows/release.yml) builds a
6-leg matrix of pre-packaged binaries that PIE picks up.

## Roadmap

**Shipped**

- Load GGUF from disk
- Fluent chat prompts (`Prompt::system()->withUser()…`)
- `Model::chat()` returning a `Response` (reasoning + answer split,
  finish-reason, token count)
- `Model::raw()` escape hatch for prompt-string control
- `Model::embed()` + `Embedding` (vector math, cosine similarity)
- Typed exception hierarchy
- PHPT suite + CI on PHP {8.3, 8.4, 8.5} × {macOS-arm64, ubuntu-latest}
- PIE-compatible composer.json + tag-triggered per-platform binary release workflow

**Next (not committed)**

- Streaming completions (PHP `Generator` or callback-based)
- Reusable session/context objects (KV-cache reuse across `chat()` calls)
- Stop-string support on `chat()` / `raw()`
- Tool calling (delegated to the model's tool-template support where present)
- Continuous batching for worker scenarios
- Apple Metal on by default for macOS-arm64

## License

MIT — see [LICENSE](LICENSE).
