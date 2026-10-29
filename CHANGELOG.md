# Changelog

All notable changes to ext-infer are documented here. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/)
(pre-1.0: breaking changes bump the minor). Dates are tag dates.

## [0.2.2] - 2026-10-29

### Fixed

- **Stubs match the extension exactly.** They declared `final` on
  Embedding, Message, Prompt and Response, which the runtime doesn't make final; `array $options = []`
  where the runtime has `?array $options = null`; and they omitted the
  constructors the runtime registers (the exception classes can't be
  constructed from PHP). Now enforced in CI on every leg
  (`make stubs-check` locally). No PHP-visible change: the extension
  behaves exactly as before; only IDE / static-analysis users see the
  corrected signatures.

### Changed

- `serde_json` 1.0.150 → 1.0.154 (Dependabot).
- CI: GitHub Actions on their Node 24 majors (Dependabot).

## [0.2.1] - 2026-10-29

Maintenance release: a current llama.cpp, one crash fix, no new API.

### Added

- **Newer model architectures load.** The bundled llama.cpp moves from
  April to mid-October 2026. Verified, and failing to load on 0.2.0:
  **Nanbeige4.2** (`nanbeige`, llama.cpp #25994): `chat()`, the
  reasoning split, and `schema` output. **Spark-X2.5** (`spark2_5`,
  #27868): loads and runs `raw()`; its Jinja-only chat template isn't
  recognised by llama.cpp's built-in template matcher, so `chat()`
  throws `apply_chat_template failed` for it. See
  [Choosing a model](https://infer.displace.tech/guide/models.html).

### Fixed

- **Long prompts no longer abort the PHP process.** llama.cpp's context
  `n_batch` defaulted to 2048 and `llama_decode` asserts (`abort()`, not
  an error code) when a batch exceeds it. Any `chat()`/`raw()` prompt
  between 2049 tokens and `nCtx`, and any `RerankModel` query + document
  over 2048 tokens with the default `n_ctx` of 4096, killed the worker.
  `n_batch` now matches `n_ctx`; memory use is unchanged.

### Changed

- `llama-cpp-2` `=0.1.146` → `=0.1.160`. Internal adaptation only (the
  tokenizer API moved to `LlamaVocab`); prompts containing a NUL byte
  still throw `InferenceException` with the same message. Greedy chat,
  raw, grammar, embedding (last- and mean-pooled) and reranker output
  were re-verified byte-identical to 0.2.0 on the Qwen3 0.6B reference
  models.
- `ext-php-rs` `=0.15.13` → `=0.16.3`. Two PHP-visible effects:
  - Passing an argument of the wrong type (e.g. `maxTokens: 'x'`) now
    throws `\TypeError`, like PHP's own internal functions, instead of a
    plain `\Exception`.
  - The trailing `?array $options` parameters now report their `null`
    default through Reflection (`isDefaultValueAvailable()`), matching
    the stubs. Signatures are otherwise identical.
- CI runs weekly as well as on push, tests PHP 8.5 on macOS arm64 again,
  adds ZTS PHP 8.4/8.5 legs on Linux, and pins bindgen's libclang on the
  ubuntu-26.04 runners. Dependabot keeps the non-upstream crates
  current.

### Documentation

- `chat.md` explains the unrecognised-template case of
  `apply_chat_template failed`.
- Qwen3 has no 7B model; the dead `Qwen3-7B-Instruct` links now point at
  the official `Qwen/Qwen3-8B-GGUF`.
- Dropped the "v0.1.0 is still in flight" pre-release notes.

## [0.2.0] - 2026-06-11

### Added

- Grammar-constrained generation: `options: ['schema' => …]` (JSON
  Schema as a PHP array or JSON string) or `['grammar' => …]` (raw GBNF)
  on `chat()` and `raw()`. The schema subset is strict: unsupported
  keywords throw rather than silently under-constraining.
- `RerankModel` for the Qwen3-Reranker family: `score()` returns the
  calibrated `P(yes)/(P(yes)+P(no))`; `rank()` returns best-first
  `['index', 'score']` rows in the ai-contracts `Reranker` shape.
- `Embedding::packed()`: the vector as a packed little-endian float32
  string, never materialised as PHP floats.
- `THIRD-PARTY-NOTICES.md` and a transitive `cargo about` license
  manifest attached to every release.

### Fixed

- Prompt and embedding batches are sized to the input; a fixed
  512-token batch silently capped prompt length.
- Removed a double `accept()` in the sampling loop: harmless with
  stateless samplers, a `GGML_ASSERT` abort with the grammar sampler.

## [0.1.1] - 2026-06-06

### Fixed

- PIE installs on macOS: the in-archive binary is now `infer.so` on
  every platform (0.1.0 shipped `infer.dylib`, which PIE can't find).
  Packagist's immutable stable tags forced a version bump rather than a
  retag; 0.1.0 is superseded.

## [0.1.0] - 2026-06-06

First public release.

### Added

- `Model::load()` / `close()`; `chat()` with the immutable `Prompt`
  builder rendered through the model's embedded chat template; `raw()`
  completions; `embed()` returning `Embedding` (`dimensions()`,
  `normalize()`, `cosineSimilarity()`).
- `Response` with the automatic `<think>…</think>` split
  (`answer()` / `reasoning()`), `finishReason()`, `tokensGenerated()`.
- Typed exceptions under `InferException` (extends `\RuntimeException`).
- PHPT suite, CI matrix (PHP 8.3/8.4/8.5 × macOS arm64 / Linux x86_64 /
  Linux arm64), PIE-compatible `composer.json`, tag-triggered binary
  releases, documentation at infer.displace.tech.

### Fixed

- Release tarballs use PIE's exact filename convention.

## [0.1.0-rc.1] - 2026-06-06

Release candidate of the 0.1.0 surface, used to exercise the release
workflow.

[0.2.2]: https://github.com/DisplaceTech/ext-infer/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/DisplaceTech/ext-infer/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/DisplaceTech/ext-infer/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/DisplaceTech/ext-infer/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/DisplaceTech/ext-infer/compare/v0.1.0-rc.1...v0.1.0
[0.1.0-rc.1]: https://github.com/DisplaceTech/ext-infer/releases/tag/v0.1.0-rc.1
