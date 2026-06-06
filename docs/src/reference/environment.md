# Environment variables

The extension reads exactly one environment variable today. We'll add
more as they earn their keep; the conservative approach is to keep
configuration in PHP (named arguments, load options) rather than
sprinkled across the environment.

## `EXT_INFER_LOG`

Restores llama.cpp's verbose stderr logging, which is silenced by
default.

| Value     | Effect                                                             |
| --------- | ------------------------------------------------------------------ |
| *(unset)* | llama.cpp logs are silenced. This is the default.                  |
| Any value | llama.cpp logs are passed through to stderr verbatim.              |

```sh
EXT_INFER_LOG=1 php hello.php
```

### Why silence by default?

A single `Model::load()` + `chat()` pair against a typical GGUF
produces several hundred lines of stderr — model metadata, KV-cache
sizing, graph reservation, attention layout, sampler config, and
more. For a CLI tool drilling into a problem it's useful; for a PHP
extension running inside a request, it's structured-log poison.

### When to enable it

- Diagnosing a `ModelLoadException`. The verbose log dumps the GGUF
  header before failing, which usually points at the cause (wrong
  architecture, wrong quant, truncated file).
- Diagnosing a slow load. The log shows where the time goes —
  reading from disk, mmap setup, weight copy.
- Reporting an issue. The first thing maintainers will ask for is the
  verbose log; capture it once with `EXT_INFER_LOG=1` and paste.

### How it works

The extension hooks `llama_log_set` at backend init time, replacing
llama.cpp's default callback with a no-op. The hook is process-global —
once installed, it covers every subsequent call. `EXT_INFER_LOG` is
checked only at backend init (the first time `Model::load()` is
called); changing the variable mid-process has no effect.

## Reserved for future use

These names are not consumed by the extension today but may be in
future versions. Avoid using them as application env vars to keep your
forward-upgrade path clean:

- `EXT_INFER_DEFAULT_NCTX`
- `EXT_INFER_DEFAULT_TEMPERATURE`
- `EXT_INFER_BACKEND` (CPU / Metal / CUDA selection at runtime)

If you want any of these to land sooner rather than later, open an
[issue](https://github.com/DisplaceTech/ext-infer/issues) with the
use case.
