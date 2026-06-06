# ext-infer — MVP plan

This document tracks what's left between the current scaffolding and the
first publishable cut of `ext-infer`. Phase scope is defined in
[`README.md`](README.md); this file is the milestone-by-milestone path
through it, plus the questions still open before each milestone closes.

## Status snapshot

| Deliverable                                | Status | Notes                                                                                                          |
| ------------------------------------------ | ------ | -------------------------------------------------------------------------------------------------------------- |
| `Cargo.toml` w/ pinned deps                | done   | `ext-php-rs 0.15.13`, `llama-cpp-2 0.1.146`, `thiserror 2.0.18`. Toolchain pinned at `1.88.0`.                  |
| `src/error.rs` exception hierarchy         | done   | `InferException : \RuntimeException`, `ModelLoadException`, `InferenceException`.                              |
| `src/model.rs` public API                  | done   | `Model::load(string, ?array): self`, `Model::complete(string, ?array): string`, `Model::close(): void`.        |
| `src/lib.rs` module registration           | done   | Module name overridden to `infer`; parent class registered before subclasses.                                  |
| `stubs/infer.stubs.php`                    | done   | Hand-authored to match the registered classes. Should be re-generated via `make stubs` before tagging.         |
| PHPT tests                                 | done   | 2 model-free tests pass; 4 model-dependent tests skip unless `INFER_TEST_MODEL` is set.                        |
| `README.md` w/ pitch, build, example       | done   |                                                                                                                |
| `Makefile` (build/test/install/clean/...)  | done   | macOS/Linux suffix detection via `uname`; `cargo-php` discoverability check.                                   |
| `.github/workflows/ci.yml`                 | done   | Lint job + matrix `{ubuntu-latest, macos-14} × {8.3, 8.4}`. Fetches matching `run-tests.php` per PHP version.  |
| `.cargo/config.toml` linker flags          | done   | `-Wl,-undefined,dynamic_lookup` on non-Windows; `-C target-feature=-crt-static` on musl.                       |
| Quality bar: `cargo clippy -D warnings`    | passing |                                                                                                                |
| Quality bar: `cargo fmt --check`           | passing |                                                                                                                |
| Quality bar: no reachable `TODO!`/`unimplemented!()` | passing | Phase-1 surface is fully implemented.                                                                |

## Milestones

### M0 — scaffolding green (DONE)

- [x] `cargo check` clean.
- [x] `cargo clippy --all-targets -- -D warnings` clean.
- [x] `cargo fmt --all --check` clean.
- [x] `cargo build` produces `target/debug/libinfer.{dylib,so}`.
- [x] `php -d extension=...libinfer.dylib -r '...'` loads, class hierarchy
      verified, `Model::__construct()` and missing-file `Model::load()`
      both throw the right subclass.
- [x] `make test` runs the PHPT suite; 2 pass, 4 skip without a model.

### M1 — real-model smoke (macOS-arm64)

Goal: prove the inference loop produces sensible output against an actual
GGUF file, with both greedy and sampled paths exercised.

- [x] Pick a canonical test model. Qwen3-0.6B-Q8_0 (~640 MB,
      Apache-2.0-licensed via `Qwen/Qwen3-0.6B-GGUF` on HuggingFace) is
      small enough to fit in a developer's `models/` directory without
      thinking about disk; the README points contributors at it with a
      one-line `curl`.
- [x] `INFER_TEST_MODEL=models/Qwen3-0.6B-Q8_0.gguf make test`
      end-to-end: all six PHPT tests pass.
- [x] `examples/hello-world.php` produces non-empty, deterministic
      output at `temperature=0.0` ("Paris. The capital of Italy is
      Rome..."), distinct-per-seed output at `temperature=0.8`.
- [x] Silence llama.cpp's stderr noise by default — see `backend()` in
      `src/model.rs`. Opt back in with `EXT_INFER_LOG=1`. Without this,
      the README's "hello world" was unusable.
- [ ] Verify `make install` ships `infer.dylib` into the homebrew
      `extension-dir` and `php -m | grep infer` finds it.
- [ ] Verify the `metal` feature (`make release FEATURES=metal && make
      install FEATURES=metal`) accelerates inference on M-series — record
      tokens/sec for both backends in [`README.md`](README.md).

Open questions:

- Do we ship a tiny "tinyllama"-class fixture model for CI use, or keep CI
  model-free and run model tests only locally? Leaning toward the latter
  — model bytes blow past sane CI cache budgets and licensing is murky
  for many small GGUFs. (Qwen3-0.6B-Q8 is Apache-2.0, so we *could*
  cache it in CI — revisit when the matrix grows.)

### M2 — Linux x86_64 parity

Goal: CI green across the full matrix in `.github/workflows/ci.yml`.

- [ ] Validate the Linux build path end-to-end on `ubuntu-latest`. Right
      now CI only confirms the extension loads — we don't exercise any
      `Model` method without a fixture.
- [ ] Decide on the model-dependent test strategy in CI (see M1 open
      question). If we choose to run them, plumb in a model-download step
      gated by an actions cache.
- [ ] Confirm `cargo-php install` works in the CI environment (we don't
      currently `make install` in CI; `make build && make test` only).

### M3 — Linux arm64

Goal: third leg of the supported platform triple is green.

- [ ] Add a `linux/arm64` runner to the CI matrix. Options:
      `ubuntu-24.04-arm` (GitHub-hosted, available since 2024), or QEMU
      under `docker/setup-qemu-action`. Prefer the native runner —
      llama.cpp under QEMU is intolerably slow to build.
- [ ] Confirm `llama-cpp-2`'s build script picks the right ggml CPU
      backend on arm64 (NEON, no SVE assumed).

### M4 — first publishable cut

Goal: `0.1.0` tag with a Composer manifest and PECL-style README badges.

- [ ] `composer.json` advertising `displace/ext-infer` and the
      extension's class map (stubs). Optional dev-dep on a userland
      package that smoke-tests the extension is loaded.
- [ ] Regenerate `stubs/infer.stubs.php` via `make stubs` and diff against
      the committed copy — they must match for the IDE-stub story to be
      honest.
- [ ] Tag `v0.1.0`. Do **not** publish to crates.io yet (the package is
      `publish = false` for now); we publish only once the Composer +
      docs story is settled.
- [ ] Write a short blog post / release note framing the Phase 2 roadmap
      so users know what's coming and what's deliberately missing.

### M5 — Phase 2 design (not committed)

Phase 2 is sketched in [`README.md`](README.md#roadmap). Before any of it
lands, we want a written design doc per item — the public surface shaped
here in Phase 1 should accommodate them without breaking changes:

- **Streaming completions.** Likely `Model::stream(string, array): \Generator`
  yielding strings, or a callback variant `Model::complete(..., callable)`
  that takes a `function (string $piece): bool` returning `false` to
  cancel. Generators are more idiomatic; callbacks are easier to bridge
  to async runners. Pick one before implementing.
- **Embeddings.** `Model::embed(string|array $input, array $opts): array`
  returning `array<int,array<int,float>>`. Needs a model loaded with the
  embedding-pooling option set; we'd likely add an `embedding: true` key
  to `Model::load()`'s options array.
- **Chat templates.** Probably a `ChatTemplate` class wrapping
  `llama_cpp_2`'s template-rendering helpers, plus `Model::chat(array
  $messages, array $opts): string` for the common case. Tool-calling
  comes after this.
- **Reusable session contexts.** Today every `complete()` builds and
  destroys a `LlamaContext`. Phase 2 should expose a `Session` (or
  similar) that holds a context across calls so KV-cache reuse is
  possible — this is the single biggest perf win on hand-off-style
  conversations. The current `Model` surface doesn't preclude it: we
  introduce a new class rather than mutate `Model`.
- **Continuous batching.** Server/worker scenarios. Significant API
  design exercise; punt until streaming and sessions land.
- **Metal-by-default on macOS-arm64.** Switch `default = ["metal"]` once
  the Apple Metal build is reliably green on the macOS-14 runner.

## Risks and open questions

- **`ext-php-rs` is pre-1.0** and the macro surface changes between
  point releases (eg. the `#[php(extends(...))]` form is recent). Pin
  exact versions in `Cargo.toml` (we already do) and re-pin
  deliberately on each ext-php-rs bump.
- **`llama-cpp-2` vendoring builds llama.cpp from source via cmake.**
  Cold CI builds are ~25 s on a developer laptop and several minutes on
  a fresh CI runner. Cache `target/` aggressively (we use
  `Swatinem/rust-cache@v2` already) but also consider a separately-cached
  step for the `llama-cpp-sys-2` artifacts.
- **`spl_ce_RuntimeException` is a `PHPAPI` symbol, not a documented
  contract.** It is exported by every PHP 8.x binary we care about (we
  verified PHP 8.4.20 via Homebrew), but if upstream ever marks SPL
  symbols hidden we'll need to fall back to a name-based lookup via
  `CompilerGlobals::get().class_table()`. Document this clearly in
  `src/error.rs`. (Already done.)
- **ZTS (thread-safe PHP) is untested.** Our backend singleton uses
  `std::sync::Mutex`, which is correct for ZTS, but we don't currently
  run any ZTS CI leg. ZTS is rare in modern PHP deployments, so this is
  a "fix when reported" risk rather than a blocker.
- **Apple SDK target.** On this host, cargo records `mmacosx-version-min=26.5`
  in the cmake flags, which would break loading on older macOS. The CI
  `macos-14` runner pins a reasonable floor, but `make release`
  consumers on bleeding-edge SDKs may produce binaries that don't run
  on stable Sonoma. Document `MACOSX_DEPLOYMENT_TARGET=14.0` (or
  similar) as the supported recipe.

## Working agreements

- Phase 1 surface is **frozen** for backward-compatibility purposes from
  the `0.1.0` tag onward. New options on `Model::load`/`Model::complete`
  arrays are non-breaking and welcome; renamed or removed options are
  not.
- All new public surface lands behind a PHPT test that fails before the
  implementation and passes after — even if the test is gated on
  `INFER_TEST_MODEL`.
- Every `unsafe` block carries a `// SAFETY:` comment naming the
  invariant relied on. (Currently: only `src/error.rs` has one.)
- Phase 2 work happens on feature branches; main stays releasable.
