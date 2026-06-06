# Exceptions

`ext-infer` raises exceptions for every error condition — no silent
`false` returns, no error codes. The hierarchy is small enough that
you can `catch` precisely or broadly depending on what you're after.

## Hierarchy

```
\RuntimeException
└── Displace\Infer\InferException
    ├── Displace\Infer\ModelLoadException
    └── Displace\Infer\InferenceException
```

- **`InferException`** extends PHP's `\RuntimeException`. Catching
  `\RuntimeException` in generic top-level handlers (e.g. a PSR-15
  middleware) sees every `ext-infer` error.
- **`ModelLoadException`** is raised exclusively from
  `Model::load()`.
- **`InferenceException`** is raised from `Model::chat()`,
  `Model::raw()`, `Model::embed()`, and `Embedding::cosineSimilarity()`.
- **`InferException`** itself (the base class, not just an instance
  of a subclass) is raised for "this method should never have been
  called" errors — see [Direct construction](#direct-construction).

## Which method raises what

| Method                                | Class                  | Common causes                                                 |
| ------------------------------------- | ---------------------- | ------------------------------------------------------------- |
| `Model::load()`                       | `ModelLoadException`   | Missing file, malformed GGUF, backend init failure.           |
| `Model::load()`                       | `InferException`       | Invalid option type/value (e.g. `pooling` set to `"weighted"`). |
| `Model::chat()`                       | `InferenceException`   | Model closed, no chat template, decode failure, prompt over `nCtx`. |
| `Model::raw()`                        | `InferenceException`   | Model closed, decode failure, prompt over `nCtx`.             |
| `Model::embed()`                      | `InferenceException`   | Model closed, model not loaded with `embedding: true`, decode failure. |
| `Embedding::cosineSimilarity()`       | `InferenceException`   | Dimension mismatch between the two embeddings.                |
| `new Model()` / `new Prompt()` / `new Message()` / `new Response()` / `new Embedding()` | `InferException` | Direct construction is refused; use the appropriate factory. |

## Direct construction

`Model`, `Prompt`, `Message`, `Response`, and `Embedding` all refuse
direct `new`. Each throws `InferException` (the base class) with a
hint pointing at the right factory:

```php
new Embedding();
// Displace\Infer\InferException:
//   Displace\Infer\Embedding is produced by Model::embed();
//   do not instantiate directly
```

This is deliberate: a `new Embedding()` from PHP code could lie about
which model produced it and what pooling strategy was applied — silent
mistakes that are hard to debug later. Forcing factory construction
keeps the invariants tight.

## Catching strategies

### Catch broadly at the top

For a request handler that wants to convert any `ext-infer` failure
into a 5xx response:

```php
try {
    $reply = $model->chat($prompt);
} catch (\Displace\Infer\InferException $e) {
    $log->error('inference failed', ['error' => $e->getMessage()]);
    return new Response(500, [], 'Inference temporarily unavailable.');
}
```

### Distinguish load failures from inference failures

For a CLI tool that wants different exit codes:

```php
try {
    $model = Model::load($path);
} catch (\Displace\Infer\ModelLoadException $e) {
    fwrite(STDERR, "model: " . $e->getMessage() . PHP_EOL);
    exit(2);
}

try {
    $r = $model->chat($prompt);
} catch (\Displace\Infer\InferenceException $e) {
    fwrite(STDERR, "inference: " . $e->getMessage() . PHP_EOL);
    exit(3);
}
```

### Retry vs surface

`InferenceException` covers two flavors of failure:

- **Transient** — out-of-memory under load, e.g. `with_mlock` + a
  large prompt. Often resolved by reducing `nCtx` or splitting the
  work.
- **Permanent** — model has no chat template, prompt has null bytes,
  invalid option. Retrying makes no sense.

The message string is the only signal you have today; structured error
codes are on the roadmap. For now, a pragmatic split:

```php
try {
    $r = $model->chat($prompt, maxTokens: $budget);
} catch (\Displace\Infer\InferenceException $e) {
    if (str_contains($e->getMessage(), 'n_ctx')) {
        // prompt too long — surface to caller, don't retry
        throw $e;
    }
    // other inference failure — log + maybe retry
    $log->warning('chat failed, retrying once', ['error' => $e->getMessage()]);
    $r = $model->chat($prompt, maxTokens: $budget);
}
```

## Always-safe patterns

`Model::close()` is **idempotent** — calling it on an already-closed
model is a no-op. Safe inside `finally`:

```php
$model = Model::load($path);
try {
    return $model->chat($prompt);
} finally {
    $model->close();
}
```

After `close()`, every other method on that `Model` raises
`InferenceException` with `"model has been closed"`.
