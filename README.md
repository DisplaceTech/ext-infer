<h1 align="center">ext-infer</h1>

<p align="center">
  <strong>Local LLM inference for PHP, in-process.</strong><br>
  Chat, embeddings, and reasoning models — no Python sidecar, no remote API.
</p>

<p align="center">
  <a href="https://github.com/DisplaceTech/ext-infer/actions/workflows/ci.yml"><img src="https://github.com/DisplaceTech/ext-infer/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/DisplaceTech/ext-infer/releases/latest"><img src="https://img.shields.io/github/v/release/DisplaceTech/ext-infer?sort=semver&include_prereleases" alt="Latest release" /></a>
  <img src="https://img.shields.io/badge/PHP-8.3%20%7C%208.4%20%7C%208.5-777BB4?logo=php&logoColor=white" alt="PHP 8.3 / 8.4 / 8.5" />
  <img src="https://img.shields.io/badge/Status-pre--release-orange" alt="Pre-release" />
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green" alt="MIT License" /></a>
  <a href="https://infer.displace.tech"><img src="https://img.shields.io/badge/docs-infer.displace.tech-blue" alt="Documentation" /></a>
</p>

---

## What is ext-infer?

`ext-infer` is a PHP 8.3+ extension that loads a GGUF model and runs
inference *in the PHP process* via [llama.cpp](https://github.com/ggerganov/llama.cpp).
PHP-native semantic search, RAG pipelines, and CLI/worker inference work
without shelling out to Python or hitting a remote API.

Written in Rust on top of [`ext-php-rs`](https://github.com/davidcole1340/ext-php-rs)
and the [`llama-cpp-2`](https://crates.io/crates/llama-cpp-2) bindings. The
public PHP surface is fluent and role-aware — building a chat prompt looks
like `Prompt::system(...)->withUser(...)`, not a string of `<|im_start|>`
tokens.

- 💬 **Chat completions** via an immutable `Prompt` builder that renders through the model's embedded template — no manual `<|im_start|>` plumbing.
- 🧠 **Reasoning-model aware** — `Response::answer()` and `Response::reasoning()` split Qwen3 / R1-style `<think>…</think>` output automatically.
- 📊 **Embeddings** — `Model::embed()` returns an `Embedding` with `dimensions()`, `normalize()`, `cosineSimilarity()` built in.
- ⚡ **In-process** — no subprocess fork, no IPC, no daemon. Latency is whatever the model takes to decode.
- 🛠️ **Apple Metal** acceleration is opt-in (`make release FEATURES=metal`); CPU is the portable default.
- 🧵 **Thread-safe** — `LlamaBackend` is a `Sync`-guarded singleton and each call builds its own context, so ZTS PHP + `parallel` works by design.

## Quick start

```sh
mkdir -p models
curl -L -o models/Qwen3-0.6B-Q8_0.gguf \
    https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q8_0.gguf
```

```php
<?php
use Displace\Infer\Model;
use Displace\Infer\Prompt;

$model    = Model::load('models/Qwen3-0.6B-Q8_0.gguf');
$response = $model->chat(
    Prompt::system('You are a helpful assistant.')
        ->withUser('What is 2+2?'),
    maxTokens: 256,
    temperature: 0.0,
);

echo $response->answer(), PHP_EOL;   // "2 + 2 equals 4."
echo $response->reasoning() ?? '';    // captured <think>…</think>, if any

$model->close();
```

```sh
make build       # produces target/debug/libinfer.{so,dylib}
php -d extension=$PWD/target/debug/libinfer.dylib hello.php
```

Full walkthrough — including the [interactive Symfony Console
chat](examples/chat-interactive/) and [pairwise-similarity embedding
example](examples/embedding.php) — under [**`examples/`**](examples/).

## Documentation

[**infer.displace.tech**](https://infer.displace.tech) hosts the full
guide:

- [Getting started](https://infer.displace.tech/getting-started/installation.html) — install via PIE or from source, verify, troubleshoot.
- [Guide](https://infer.displace.tech/guide/prompts.html) — prompts, chat, raw, embeddings, choosing a model.
- [Recipes](https://infer.displace.tech/recipes/multi-turn-chat.html) — multi-turn chat, semantic search, RAG over markdown, worker pools.
- [Reference](https://infer.displace.tech/reference/api.html) — full API surface, exceptions, environment variables, compatibility matrix.
- [Advanced](https://infer.displace.tech/advanced/threading.html) — threading, Metal, performance tuning.

The site is built from [`docs/`](docs/) with [`mdbook`](https://rust-lang.github.io/mdBook/)
and deploys automatically on every push to `main`.

## Compatibility

|                | macOS arm64 | Linux x86_64 | Linux arm64 | Windows |
| -------------- | :---------: | :----------: | :---------: | :-----: |
| **PHP 8.3**    |      ✅     |      ✅      |      ✅     |    —    |
| **PHP 8.4**    |      ✅     |      ✅      |      ✅     |    —    |
| **PHP 8.5**    |      ✅     |      ✅      |      ✅     |    —    |

ZTS is supported by design (the code is thread-safe), enabled in
`composer.json`, and not yet exercised in CI. Windows is intentionally
out of scope for v0.1.

## Roadmap

**Shipped** &nbsp; chat completions · raw completions · embeddings · reasoning split · typed exceptions · PHPT suite · CI matrix · PIE-compatible `composer.json` · tag-triggered binary release workflow.

**Next** &nbsp; first `v0.1.0` release · streaming completions · KV-cache reuse via reusable `Session` objects · stop-string support · tool calling · continuous batching · Apple Metal default on macos-arm64.

See [`PLAN.md`](PLAN.md) for the current planning doc and [`RELEASE.md`](RELEASE.md)
for the cut-a-release flow.

## License

[MIT](LICENSE) &copy; 2026 Eric Mann / Displace Technologies
