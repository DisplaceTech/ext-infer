<h1 align="center">ext-infer</h1>

<p align="center">
  <strong>Local LLM inference for PHP, in-process.</strong><br>
  Chat, embeddings, and reasoning models — no Python sidecar, no remote API.
</p>

<p align="center">
  <a href="https://github.com/DisplaceTech/ext-infer/actions/workflows/ci.yml"><img src="https://github.com/DisplaceTech/ext-infer/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/DisplaceTech/ext-infer/releases/latest"><img src="https://img.shields.io/github/v/release/DisplaceTech/ext-infer?sort=semver&include_prereleases" alt="Latest release" /></a>
  <img src="https://img.shields.io/badge/PHP-8.3%20%7C%208.4%20%7C%208.5-777BB4?logo=php&logoColor=white" alt="PHP 8.3 / 8.4 / 8.5" />
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green" alt="MIT License" /></a>
  <a href="https://infer.displace.tech"><img src="https://img.shields.io/badge/docs-infer.displace.tech-blue" alt="Documentation" /></a>
</p>

---

## Status

Feature-complete for its scope. Maintained: dependency updates and
new-model support ship as patch/minor releases — see the
[CHANGELOG](CHANGELOG.md). Last verified: PHP 8.3/8.4/8.5 (NTS on macOS arm64 + Linux x86_64; ZTS 8.4/8.5 on Linux), `llama-cpp-2` 0.1.160 (llama.cpp, October 2026), Qwen3 0.6B reference models, 2026-10-29.

Remaining ideas are tracked as [`future` issues](https://github.com/DisplaceTech/ext-infer/issues?q=is%3Aissue+is%3Aopen+label%3Afuture).
**Co-maintainers welcome** — if you run this in production and want a
say in where it goes, open an issue or a PR.

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
- 🧱 **Structured output** — pass a JSON Schema (or raw GBNF grammar) and sampling is constrained so malformed output is *impossible*, not retried. A 0.6B model becomes a dependable extractor.
- 🧠 **Reasoning-model aware** — `Response::answer()` and `Response::reasoning()` split Qwen3 / R1-style `<think>…</think>` output automatically.
- 📊 **Embeddings** — `Model::embed()` returns an `Embedding` with `dimensions()`, `normalize()`, `cosineSimilarity()`, and `packed()` (zero-copy handoff to vector indexes) built in.
- 🎯 **Reranking** — `RerankModel` scores (query, document) pairs through Qwen3-Reranker's calibrated yes/no judgment; completes the embed → rerank two-stage retrieval pipeline.
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

ZTS is supported by design (the code is thread-safe) and enabled in
`composer.json`; CI runs the PHPT suite on ZTS PHP 8.4 and 8.5 (Linux)
every week. Prebuilt release binaries are NTS; ZTS installs build from
source through PIE. Windows is intentionally out of scope.

## Roadmap

**Shipped** &nbsp; chat completions · raw completions · grammar/JSON-Schema constrained generation · embeddings (+ packed float32 output) · `RerankModel` · reasoning split · typed exceptions · PHPT suite · CI matrix · PIE-compatible `composer.json` · tag-triggered binary release workflow · THIRD-PARTY-NOTICES + `cargo about` license manifest · a current llama.cpp with newer architectures (0.2.1).

**Remaining ideas** live as [`future` issues](https://github.com/DisplaceTech/ext-infer/issues?q=is%3Aissue+is%3Aopen+label%3Afuture), each with its reasoning and any locked constraint (streaming, for one, is deliberately v0.3+). [`PLAN.md`](PLAN.md) holds the design notes and decision history; [`RELEASE.md`](RELEASE.md) is the cut-a-release flow.

## Deliberately out of scope

**Training and fine-tuning** · **GPU-first serving** (CPU is the
portable default; Metal is opt-in) · **frontier-model-class inference**
— the target is small local models on a 4GB+ box · **an orchestration
framework** — chains and agents belong to the frameworks integrating
[ai-contracts](https://github.com/DisplaceTech/ai-contracts) ·
**Windows**. (Platform-wide non-goals; see the umbrella
ARCHITECTURE.md.)

## Part of a stack

Transcribe → chunk → embed → search → answer, entirely on your hardware:

| Stage | Package | What it does |
| ----- | ------- | ------------ |
| Transcribe | [ext-whisper](https://github.com/DisplaceTech/ext-whisper) | Speech-to-text over whisper.cpp |
| Chunk | [ai-toolkit](https://github.com/DisplaceTech/ai-toolkit) | Text chunkers and packed-vector math, pure PHP |
| Embed · rerank · answer | **ext-infer** (this repo) | Local LLM chat, embeddings and reranking over llama.cpp |
| Search | [ext-turbovec](https://github.com/DisplaceTech/ext-turbovec) | Compressed in-memory vector search (TurboQuant) |
| Integrate | [ai-contracts](https://github.com/DisplaceTech/ai-contracts) | The interfaces frameworks code against |
| Reference app | [rag-over-markdown](https://github.com/DisplaceTech/rag-over-markdown) | The whole pipeline over a folder of markdown |

Every package links the others; the extensions share one scaffold,
[ext-template](https://github.com/DisplaceTech/ext-template).

## Writing about this

- [ext-infer: Local LLM Inference, Natively in PHP](https://eric.mann.blog/ext-infer-local-llm-inference-php/)
- [The Case for Local AI: Keep Your Data and Your Models Close](https://eric.mann.blog/case-for-local-ai/)
- [Meaning as Geometry: What Embeddings Actually Are](https://eric.mann.blog/meaning-as-geometry-what-embeddings-actually-are/)
- [From Voice to Vectors: Building a Searchable Audio Archive in PHP](https://eric.mann.blog/voice-to-vectors-audio-search-php/)

The stack's launch series ran Sept 22 – Oct 1, 2026; everything else
is at [eric.mann.blog/writing](https://eric.mann.blog/writing/).

## License

[MIT](LICENSE) &copy; 2026 Eric Mann / Displace Technologies
