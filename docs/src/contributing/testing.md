# Testing

`ext-infer` has two test layers:

- **PHPT** — integration tests that exercise the extension from PHP.
  This is where the real correctness coverage lives.
- **Rust unit tests** — for pure-Rust helpers (currently none;
  see [Why no Rust unit tests?](#why-no-rust-unit-tests) below).

Plus formatting and clippy. CI runs all of the above on every push.

## Running PHPT locally

The test harness lives in [`tests/phpt/`](https://github.com/DisplaceTech/ext-infer/tree/main/tests/phpt).
`make test` runs the full suite against a debug build:

```sh
make test
```

What that command actually does:

1. Build (`cargo build`).
2. Sanity-load — confirm the extension actually loaded into PHP.
3. Fetch `run-tests.php` from PHP-src matching the current minor (if
   not already cached).
4. Run `php run-tests.php -q --show-diff tests/phpt/` with
   `TEST_PHP_EXECUTABLE` and `TEST_PHP_ARGS` set so the freshly built
   `.so` / `.dylib` is loaded.

Tests gated on a real model use the `INFER_TEST_MODEL` environment
variable:

```sh
INFER_TEST_MODEL=$PWD/models/Qwen3-0.6B-Q8_0.gguf make test
```

Without the variable, model-gated tests skip cleanly. CI runs in this
"no model" mode by default; setting `INFER_TEST_MODEL` runs the full
suite.

### Writing a PHPT test

Files in `tests/phpt/` follow the standard PHPT format:

```phpt
--TEST--
Model::chat() returns a Response with the model's answer
--SKIPIF--
<?php
if (!extension_loaded('infer')) {
    echo 'skip ext-infer not loaded';
    exit;
}
$path = getenv('INFER_TEST_MODEL');
if (!$path || !is_file($path)) {
    echo 'skip INFER_TEST_MODEL not set to an existing GGUF file';
}
?>
--FILE--
<?php
$model = \Displace\Infer\Model::load(getenv('INFER_TEST_MODEL'));
$r = $model->chat(\Displace\Infer\Prompt::user('hi'), maxTokens: 32);
echo $r->finishReason() === 'eos' || $r->finishReason() === 'length' ? "ok\n" : "bad\n";
$model->close();
?>
--EXPECT--
ok
```

Filename convention: `NNN-short-description.phpt`. NNN ordering is
loose — it determines the order `run-tests.php` runs them in, which
doesn't really matter.

Three sections every model-gated test needs:

- **`--SKIPIF--`** — `skip` if `extension_loaded('infer')` is false
  (the harness invocation always passes `-d extension=…`, so this
  catches setup mistakes) and skip if `INFER_TEST_MODEL` is unset.
- **`--FILE--`** — the actual PHP under test.
- **`--EXPECT--`** or **`--EXPECTF--`** — expected output. Use
  `--EXPECTF--` if you need wildcards (`%s`, `%d`).

For tests that DON'T need a model, drop the `INFER_TEST_MODEL` check
from `--SKIPIF--`. They'll run in CI's no-model leg.

## Running Rust unit tests

```sh
cargo test --lib
```

…would be the command, but see the next section.

### Why no Rust unit tests?

Earlier versions had Rust unit tests in `src/response.rs` and
`src/embedding.rs` covering pure-Rust helpers. They were dropped
because `cargo test --lib` builds an executable that statically links
the crate, which pulls in references to the ext-php-rs runtime
symbols (`zend_throw_exception`, `_emalloc`, ...) — symbols only
resolved when loaded into a real PHP host. On a clean checkout,
`cargo test --lib` fails to link.

PHPT covers the same correctness ground end-to-end, so this is a net
win for CI simplicity. If a pure-Rust helper grows complex enough to
warrant unit tests in isolation, the path forward is to factor it
into a sibling crate that has no `ext-php-rs` dependency.

## Linting

```sh
make fmt-check       # cargo fmt --all --check
make clippy          # cargo clippy --all-targets -- -D warnings
```

CI runs both with `-D warnings`. Local lints are pinned to the
same Rust toolchain as the build (via `rust-toolchain.toml`).

## CI structure

[`.github/workflows/ci.yml`](https://github.com/DisplaceTech/ext-infer/blob/main/.github/workflows/ci.yml)
runs on every push and PR:

- **`rustfmt + clippy`** on ubuntu-latest with PHP 8.4. Fast (~1
  minute warm-cache).
- **Test matrix** — 6 legs: `{ubuntu-latest, macos-14}` × `{8.3, 8.4,
  8.5}`. Each builds the extension, loads it, runs the no-model PHPT
  legs. Cache is scoped per-PHP-minor (see the comment in
  `ci.yml` about why this matters for `ext-php-rs` binding regeneration).

What CI does **not** do:

- Run model-gated PHPT tests. Adding a fixture model to CI is on the
  [roadmap](https://github.com/DisplaceTech/ext-infer/blob/main/PLAN.md);
  for now, run them locally before tagging.
- Exercise ZTS PHP. See [Threading & ZTS](../advanced/threading.md#future-work).

## Pre-flight checklist

Before opening a PR, the maintainers run:

```sh
cargo fmt --all --check                              # no diff
cargo clippy --all-targets -- -D warnings            # clean
INFER_TEST_MODEL=$PWD/models/Qwen3-0.6B-Q8_0.gguf make test  # all green
```

If any of those fail, the PR will fail CI for the same reason — fix
locally first.

## Next

- [Releasing](./releasing.md) — what runs in the *release* workflow
  (a different beast than CI).
- [Building from source](./building.md) — getting to the point where
  `make test` can even run.
