# Threading & ZTS

`ext-infer` is **thread-safe by design**. This page documents what
that actually means: where the synchronization happens, what the
runtime expectations are, and where the rough edges still are.

## The thread-safety story, top to bottom

### 1. `LlamaBackend` is a `Sync`-guarded singleton

`llama.cpp`'s `LlamaBackend::init()` is process-global state.
Initializing it twice is undefined behavior; not initializing it at
all means no inference. `ext-infer` resolves this with:

```rust
static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
static BACKEND_INIT: Mutex<()> = Mutex::new(());
```

The first `Model::load()` call (from any thread) acquires the mutex,
checks the `OnceLock`, calls `LlamaBackend::init()` if needed, and
publishes the result. Every subsequent call sees a populated
`OnceLock` and returns immediately without re-acquiring. The mutex is
contended only during cold startup.

`OnceLock<T>` is `Sync` as long as `T: Send + Sync`, which
`LlamaBackend` is.

### 2. `LlamaModel` weights are immutable after load

llama.cpp explicitly supports running multiple contexts in parallel
against a single loaded model. The weights are read-only after `load_from_file`
returns; only the per-context state (KV cache, sampler state) mutates
during inference.

This is what makes the "load once, use from many threads" pattern
work without any locking on the model itself.

### 3. Per-call `LlamaContext`

`Model::chat()`, `Model::raw()`, and `Model::embed()` each build a
**fresh** `LlamaContext` for the duration of the call and drop it on
the way out. Two threads calling `chat()` simultaneously get two
independent contexts that share the same underlying weights via
references.

```rust
// Inside run_completion:
let ctx_params = LlamaContextParams::default().with_n_ctx(Some(n_ctx));
let mut ctx = model.new_context(backend, ctx_params)?;
// ... decode, sample, decode, sample ...
// ctx dropped at function exit
```

No state survives the call. No cleanup is required. No two threads
ever touch the same `LlamaContext`.

### 4. `Model::close()` is the one `&mut self` method

PHP's runtime serializes calls into the same object method via its
own object lock, so `close()` from one thread while another calls
`chat()` should be safe by the runtime's invariants — but it's the
one place where the Rust code mutates the `Model` itself
(`self.inner = None`). The worst case is the user-after-close error,
which is what `close()` is supposed to provoke anyway.

## When you actually get concurrency

Three deployment shapes use this thread-safety:

- **PHP-FPM workers** (process-based) — each worker is independent;
  the thread-safety story doesn't matter, but the *mmap-sharing*
  story does. See [Worker pools](../recipes/worker-pools.md).
- **ZTS PHP + `parallel`** (thread-based) — one PHP process,
  multiple OS threads, each calling `chat()` on a shared `Model`.
  This is what the thread-safety story is for.
- **Swoole / ReactPHP coroutines** (single-threaded but
  context-switching) — not actually concurrent at the OS level, so
  thread-safety isn't strictly required; you'll still benefit from
  the per-call context pattern because no global state survives.

## ZTS-specific notes

ZTS (Zend Thread Safe) is a PHP build mode that adds TLS storage
around engine globals so multiple PHP interpreters can run in one
process. It's required for [`pthreads`](https://www.php.net/manual/en/book.pthreads.php)
(EOL) and the more modern [`parallel`](https://www.php.net/manual/en/book.parallel.php)
extension.

### Detecting ZTS

```sh
php -i | grep 'Thread Safety'
# expected: Thread Safety => enabled
```

Or from PHP:

```php
if (PHP_ZTS) {
    // ZTS build
}
```

### Installing ZTS PHP

Most distros ship NTS PHP. To get ZTS:

- **Ubuntu / Debian**: build from source with `./configure --enable-zts`.
  Some PPAs (`ondrej/php`) ship a ZTS variant under `php{X}.{Y}-zts`
  but coverage is spotty.
- **macOS**: Homebrew's `php@*` formulas are NTS. Use
  `phpbrew install +zts +parallel` or build from source.
- **Docker**: official `php:*-cli` images are NTS. The community
  `silkeh/php` images include ZTS variants.

`ext-infer` v0.1 ships **NTS-only release binaries**. ZTS users need
to build from source. The composer.json declares
`support-zts: true` so a future ZTS release can ship without changing
the install story.

### Loading `ext-infer` into ZTS PHP

Same `extension=infer` line in `php.ini`, plus `parallel` if you want
threading:

```ini
extension=infer.so
extension=parallel.so
```

### A minimal `parallel` test

```php
<?php
use parallel\Runtime;
use Displace\Infer\Model;
use Displace\Infer\Prompt;

$model = Model::load('models/Qwen3-0.6B-Q8_0.gguf');

$rt1 = new Runtime();
$rt2 = new Runtime();

$f1 = $rt1->run(function (Model $m) {
    return $m->chat(Prompt::user('What is the capital of France?'))->answer();
}, [$model]);
$f2 = $rt2->run(function (Model $m) {
    return $m->chat(Prompt::user('What is the capital of Italy?'))->answer();
}, [$model]);

echo "F: ", $f1->value(), PHP_EOL;
echo "I: ", $f2->value(), PHP_EOL;

$model->close();
```

If this works, you have concurrent inference. If it crashes — please
[open an issue](https://github.com/DisplaceTech/ext-infer/issues) with
the model name, PHP version, build flags, and the crash output. CI
doesn't exercise this path yet, so user reports are the canary.

## Future work

Two threading-related items on the roadmap:

### CI exercise for ZTS

Add a `parallel`-driven stress test to CI. Today the matrix only
covers NTS. Adding ZTS will require:

- Building a ZTS-PHP runner image (the maintainers haven't picked
  one yet).
- Adding a ZTS leg to `ci.yml` and the release matrix in
  `release.yml`.

### Reusable session contexts

Today, every `chat()` call rebuilds the `LlamaContext` from scratch.
That drops the KV cache, so multi-turn conversations re-prefill on
every turn. A `Session` abstraction that owns a long-lived context
would let users opt into KV-cache reuse for back-to-back turns of the
same conversation. Tracked in [`PLAN.md`](https://github.com/DisplaceTech/ext-infer/blob/main/PLAN.md).

This wouldn't change the thread-safety story — each `Session` would
be owned by one thread (or guarded by a mutex if shared) — but it
would significantly improve multi-turn performance.

## Next

- [Worker pools recipe](../recipes/worker-pools.md) — practical
  patterns for concurrency in production.
- [Performance tuning](./performance.md) — knobs that matter once
  you've got the concurrency story right.
