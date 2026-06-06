# ext-infer — plan

Living document. Each section answers "what's left, and why does it
matter." Project status / surface description lives in
[`README.md`](README.md); how-to-cut-a-release lives in
[`RELEASE.md`](RELEASE.md).

## Status snapshot

| Surface                                | Status   |
| -------------------------------------- | -------- |
| `Model::load` + load options           | shipped  |
| `Model::close` (idempotent)            | shipped  |
| `Model::chat(Prompt, …): Response`     | shipped  |
| `Model::raw(string, …): string`        | shipped  |
| `Model::embed(string): Embedding`      | shipped  |
| `Prompt` (immutable builder)           | shipped  |
| `Message` (read-only)                  | shipped  |
| `Response` (reasoning/answer split)    | shipped  |
| `Embedding` (vector math)              | shipped  |
| `InferException` hierarchy             | shipped  |
| PHPT suite (11 tests, 7 model-gated)   | shipped  |
| CI matrix (8.3/8.4 × {macos, ubuntu})  | shipped  |
| `composer.json` (PIE-compatible)       | shipped  |
| Tag-triggered binary release workflow  | shipped, untested |
| `RELEASE.md`                           | shipped  |
| `examples/` (3 examples + READMEs)     | shipped  |
| Apple Metal feature                    | available, not default |
| ZTS-PHP support                        | enabled in composer.json, untested in CI |

## Up next

### Cut v0.1.0

The first release exercises the release pipeline end-to-end and is
the milestone for "stop relying on `-d extension=…` for everything."

- [ ] Bump `Cargo.toml` to `0.1.0`.
- [ ] Tag `v0.1.0`, push the tag.
- [ ] Verify all six matrix legs of `.github/workflows/release.yml`
      produce tarballs with the right PIE filenames + `.sha256`
      sidecars.
- [ ] Manually edit + publish the draft Release notes.
- [ ] `pie install displace/ext-infer` on a clean box; confirm
      `php -m | grep infer` and run `examples/hello-world.php`.
- [ ] Document any issues found back into `RELEASE.md`.

### ZTS exercise

ZTS support is declared in `composer.json` because the code is
thread-safe by design — but neither the CI matrix nor the release
matrix builds against a ZTS-PHP runner today.

- [ ] Add an `ubuntu-latest` ZTS leg to `ci.yml`. Will need a custom
      PHP install (apt's `php-zts` package, or
      [`shivammathur/setup-php`](https://github.com/shivammathur/setup-php)
      if it has grown a `ts: true` option since we last checked).
- [ ] Once green, add ZTS legs to `release.yml` so PIE can serve the
      right tarball to ZTS users.
- [ ] Stress test: a `parallel\Runtime` script that loads one `Model`
      and runs N concurrent `chat()` calls. Verify no crash, no
      response cross-contamination.

### macOS Metal default

`make release FEATURES=metal` already works. Two things gate making it
the default for macos-arm64 release tarballs:

- [ ] Verify the macos-14 GitHub runner's hardware actually
      benefits from Metal (the runner family changes; some
      revisions have GPU support and others don't).
- [ ] Decide what to ship: a Metal-only macos arm64 tarball, or two
      tarballs (`-metal` suffix + plain CPU) and let PIE pick. The
      filename convention can carry a `Debug`/`TSMode` token but
      doesn't have a slot for "accel"; we'd likely encode it in the
      `Debug` field (`metal` vs unset).

### Stop-string support

Common LLM API feature. `chat()` and `raw()` would accept a
`stop: string|array<string>` named arg; generation halts the moment
any stop-string appears in the decoded output and that prefix is
returned. Useful for:

- early-termination on a known delimiter (`"###"`, `"\n\n"`)
- letting users define their own structured templates in `raw()`
  mode

Open question: does the truncation happen at the stop-string boundary
(strict), or include the stop-string (lenient)? OpenAI excludes it;
Anthropic excludes it; llama-cli includes it. Pick strict for
consistency with the dominant API surface.

### Streaming completions

Long-running `chat()` calls are unfriendly in a request/response web
context. Two viable surfaces:

- **Generator-returning**: `Model::chatStream(Prompt, …): Generator`
  yielding `Response`-like fragments (incremental `text`, with the
  final fragment carrying `finishReason` + `tokensGenerated`).
- **Callback-returning**: `Model::chat(Prompt, …, ?Closure $onToken):
  Response` where each emitted token piece is sent through the
  closure; the closure returns `bool` to allow caller-driven
  cancellation.

Pick before implementing. Generators are more PHP-native; callbacks
are easier to bridge into async runtimes (`ReactPHP`, `Swoole`,
worker pools).

Implementation-wise this is moderate: today's decode loop in
`run_completion` already emits one token at a time — the change is
flushing each piece outward instead of accumulating it.

### Session / KV-cache reuse

Today every `chat()` call constructs a fresh `LlamaContext`. That
costs ~tens-of-ms even for cached weights, and worse, it drops the
KV cache, so multi-turn conversations re-prefill from scratch on
every turn. A `Session` object that owns a long-lived context would
make multi-turn chat dramatically faster.

API sketch:

```php
$session = $model->newSession();   // wraps a LlamaContext
$r1 = $session->chat($prompt->withUser('hi'));
$r2 = $session->chat($prompt->withUser('hi')->withAssistant($r1->answer())
                            ->withUser('and another thing'));
// On the second call, the prefix matching ($prompt + 'hi' + assistant
// reply) is already in the KV cache — only the new user message
// needs to be tokenized + decoded.
$session->close();
```

Open question: do we expose a single `Session` per model, or allow
parallel sessions (one per "conversation")? Latter is the realistic
shape — a chatbot serving many users needs one session per user.

### Tool calling

Models that ship tool-calling support (Qwen3, Llama 3.1+, Mistral
Nemo, etc.) have specific chat-template extensions that emit
function-call tokens. `llama-cpp-2` exposes `apply_chat_template_with_tools_oaicompat`
for this. Wrapping it would let PHP code register tool definitions
and dispatch on the resulting parsed call.

Bigger undertaking than the rest of this list. Sequence after
sessions land.

### Continuous batching

For worker scenarios (LLM inference as a service backed by FPM),
processing one request at a time leaves a lot of throughput on the
table. llama.cpp supports continuous batching natively; exposing it
to PHP would require a different surface (likely something like
`Model::batchChat(array<Prompt>): array<Response>`).

Probably the last item on the list — it ties into the streaming and
session decisions and shouldn't be designed in isolation.

## Operational notes

### `ext-php-rs` is pre-1.0

We pin exact versions in `Cargo.toml` (currently 0.15.13). The
`#[php(extends(...))]` and `#[php(defaults(...))]` macro syntaxes are
recent additions; both work in 0.15.13 but may shift in 0.16. Pin
deliberately on each ext-php-rs bump and run the full PHPT suite
before merging.

### `llama-cpp-2` builds llama.cpp from source

Cold CI builds are ~25 seconds on a developer M-series laptop and
several minutes on a fresh CI runner. We use `Swatinem/rust-cache@v2`
with `key: php-${{ matrix.php }}` to keep warm-cache builds under a
minute per leg.

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
