# ext-infer — design notes

Design notes and decision history: why the surface looks the way it
does, what was tried, and the constraints that bind future work. It is
not a to-do list.

- Status and surface: [`README.md`](README.md)
- What shipped, per release: [`CHANGELOG.md`](CHANGELOG.md)
- Remaining work: [`future` issues](https://github.com/DisplaceTech/ext-infer/issues?q=is%3Aissue+is%3Aopen+label%3Afuture), one per idea, each carrying
  the reasoning that used to live here
- How to cut a release: [`RELEASE.md`](RELEASE.md)

## Surface (as of 0.2.1)

| Surface                                | Status   |
| -------------------------------------- | -------- |
| `Model::load` + load options           | shipped  |
| `Model::close` (idempotent)            | shipped  |
| `Model::chat(Prompt, …): Response`     | shipped  |
| `Model::raw(string, …): string`        | shipped  |
| `Model::embed(string): Embedding`      | shipped  |
| Grammar/JSON-Schema constrained generation (`options` on chat/raw) | shipped (v0.2) |
| `Embedding::packed()` (zero-inflation float32 output) | shipped (v0.2) |
| `RerankModel` (Qwen3-Reranker yes/no logit scoring) | shipped (v0.2) |
| THIRD-PARTY-NOTICES + `cargo about` release manifest | shipped (v0.2) |
| `Prompt` (immutable builder)           | shipped  |
| `Message` (read-only)                  | shipped  |
| `Response` (reasoning/answer split)    | shipped  |
| `Embedding` (vector math)              | shipped  |
| `InferException` hierarchy             | shipped  |
| PHPT suite (16 tests, model-gated where needed) | shipped  |
| CI matrix (8.3/8.4/8.5 × {macos-arm64, ubuntu}; weekly schedule) | shipped |
| `composer.json` (PIE-compatible)       | shipped  |
| Tag-triggered binary release workflow  | shipped, exercised through v0.2.0 |
| `RELEASE.md`                           | shipped  |
| `examples/` (3 examples + READMEs)     | shipped  |
| Documentation site (infer.displace.tech) | shipped, ~20 mdbook pages |
| Apple Metal feature                    | available, not default ([#9](https://github.com/DisplaceTech/ext-infer/issues/9)) |
| ZTS-PHP support                        | PHPT suite on ZTS 8.4/8.5 in CI (0.2.1); no concurrent stress test yet ([#12](https://github.com/DisplaceTech/ext-infer/issues/12)) |

## Releases

Details per release live in [`CHANGELOG.md`](CHANGELOG.md); this table is the one-line history.

| Version | Date         | Notes                                                                                  |
| ------- | ------------ | -------------------------------------------------------------------------------------- |
| v0.2.1  | unreleased   | llama-cpp-2 0.1.160 (October 2026 llama.cpp; Nanbeige4.2 / Spark-X2.5 load), ext-php-rs 0.16.3, long-prompt abort fix (`n_batch` = `n_ctx`). |
| v0.2.0  | 2026-06-11   | Grammar/JSON-Schema constrained generation, `RerankModel` (Qwen3-Reranker), `Embedding::packed()`, THIRD-PARTY-NOTICES + `cargo about` manifest, prompt-batch sizing fix (512-token cap), sampler double-accept fix. |
| v0.1.1  | 2026-06-07   | Fixes PIE install: in-archive file is `infer.so` on every platform (not `infer.dylib` on macOS). Packagist's immutability rules forced a version bump rather than a v0.1.0 retag. |
| v0.1.0  | 2026-06-07   | First public release. Tarballs unusable via PIE due to `infer.dylib` naming on macOS — superseded by v0.1.1. |

## Remaining work

Tracked as [`future` issues](https://github.com/DisplaceTech/ext-infer/issues?q=is%3Aissue+is%3Aopen+label%3Afuture); the design reasoning moved into each
issue's body. Roughly in roadmap order:

- [#3](https://github.com/DisplaceTech/ext-infer/issues/3) Streaming completions (**deliberately v0.3+**)
- [#4](https://github.com/DisplaceTech/ext-infer/issues/4) Session / KV-cache reuse
- [#5](https://github.com/DisplaceTech/ext-infer/issues/5) Stop-string support
- [#6](https://github.com/DisplaceTech/ext-infer/issues/6) LoRA adapters
- [#7](https://github.com/DisplaceTech/ext-infer/issues/7) Multimodal (mmproj) input
- [#8](https://github.com/DisplaceTech/ext-infer/issues/8) Tool calling (now depends on #2)
- [#2](https://github.com/DisplaceTech/ext-infer/issues/2) Jinja chat-template rendering for templates llama.cpp's matcher doesn't know
- [#9](https://github.com/DisplaceTech/ext-infer/issues/9) Apple Metal by default on macOS arm64
- [#10](https://github.com/DisplaceTech/ext-infer/issues/10) Continuous batching
- [#11](https://github.com/DisplaceTech/ext-infer/issues/11) Tokenizer utilities (on demand)
- [#12](https://github.com/DisplaceTech/ext-infer/issues/12) ZTS concurrent stress test
- [#13](https://github.com/DisplaceTech/ext-infer/issues/13) x86_64 prompt-processing throughput
- [#14](https://github.com/DisplaceTech/ext-infer/issues/14) Exceptions report an empty file / line 0

## Decisions

| Date | Decision | Why |
| ---- | -------- | --- |
| 2026-10-29 | Context `n_batch` = `n_ctx` for completion, rerank and embedding contexts. | llama.cpp aborts the process (`GGML_ASSERT`) when one batch exceeds `n_batch` (default 2048); ext-infer decodes the whole prompt as one batch. `n_ubatch` keeps its default, so memory is unchanged. |
| 2026-10-29 | Keep rejecting NUL bytes in prompts after llama-cpp-2 moved to byte-slice tokenization. | Unchanged behaviour across the upgrade; a NUL in a prompt is almost always binary data passed as text. |
| 2026-10-29 | No Jinja renderer in 0.2.1, even though Spark-X2.5's template needs one. | It would change prompt rendering (and golden outputs) for models that work today; it's a feature with a dependency decision, not a patch ([#2](https://github.com/DisplaceTech/ext-infer/issues/2)). |
| 2026-06-11 | Streaming deferred to v0.3+. | Callback→generator bridging is fiddly; the embed/search/classify/extract core doesn't need it (umbrella ARCHITECTURE.md). |

## Operational notes

### `ext-php-rs` is pre-1.0

The pin lives in ext-template's `template/versions.conf` (currently
`=0.16.3`) and cascades through `bin/sync`. 0.16 converts
`#[php(defaults(...))]` values into the parameter type, so non-`i32`
numeric defaults need typed literals (`128_u32`, `0.0_f32`). On every
bump: diff a Reflection dump of the extension before/after, regenerate
stubs (`make stubs`) and diff them, and run the full PHPT suite with
models.

### `llama-cpp-2` builds llama.cpp from source

Cold CI builds are ~25 seconds on a developer M-series laptop and
several minutes on a fresh CI runner. We use `Swatinem/rust-cache@v2`
keyed on PHP version and thread-safety mode to keep warm-cache builds
under a minute per leg. The pin is exact and moves deliberately;
ext-template's upstream watcher opens an issue when crates.io moves
ahead. llama-cpp-sys-2 derives ggml's SIMD flags from Rust target
features, and ext-infer sets no `target-cpu`, so x86_64 builds use the
SSE2 baseline ([#13](https://github.com/DisplaceTech/ext-infer/issues/13)).

### `spl_ce_RuntimeException`

`InferException` extends `\RuntimeException` via a `PHPAPI`-exposed
SPL symbol that the linker resolves at extension-load time. SPL ships
in every supported PHP build, so the symbol is always present, but
it's a `PHPAPI` *symbol*, not a documented *contract*. If upstream
ever marks SPL symbols hidden, fall back to
`CompilerGlobals::get().class_table()` lookup. The current setup is
in `src/error.rs`.

### macOS deployment target

Builds on bleeding-edge macOS SDKs may produce artifacts that don't
load on older macOS releases (cmake records the runner's SDK version
in the build flags). Set `MACOSX_DEPLOYMENT_TARGET=11.0` (or whatever
floor you want to support) when building tarballs intended for wide
distribution. The release workflow inherits the GitHub runner's
default deployment target; if that becomes a problem, pin it in
`release.yml`.

## Working agreements

- Pre-1.0, breaking changes happen between minors (0.1 → 0.2), not
  patches. Once we tag `v1.0.0`, the class/method surface is frozen
  and we follow strict SemVer.
- All new public surface lands behind a PHPT test that fails before
  the implementation and passes after.
- Every `unsafe` block carries a `// SAFETY:` comment naming the
  invariant it relies on.
- Each commit is reviewable in isolation: one logical change, a
  message that says *why* (the *what* is in the diff).
