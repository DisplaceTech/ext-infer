# Options reference

Every option that any `ext-infer` method accepts, in one table per
method. For conceptual context on individual options, follow the
links in the rightmost column.

## `Model::load($path, $options)`

The second argument is an associative array. Keys are kept as snake-case
strings (like PHP ini settings) because load-time tuning is rare and the
array form composes well with config arrays loaded from disk.

| Key            | Type     | Default         | See                                       |
| -------------- | -------- | --------------- | ----------------------------------------- |
| `n_gpu_layers` | `int`    | `0`             | [Performance tuning](../advanced/performance.md) |
| `use_mmap`     | `bool`   | `true`          | [Performance tuning](../advanced/performance.md) |
| `use_mlock`    | `bool`   | `false`         | [Performance tuning](../advanced/performance.md) |
| `embedding`    | `bool`   | `false`         | [Embeddings](./embeddings.md)             |
| `pooling`      | `string` | `'unspecified'` | [Embeddings](./embeddings.md#pooling)     |

### Validation rules

- Unknown keys are *not* rejected — they're silently ignored. This is
  deliberate (forward-compatibility for callers loading config from
  files), but it means typos will be silent. If you suspect a typo,
  verify with `var_dump` against the same string before reporting a
  bug.
- Type mismatches *are* rejected, with a clear message:
  `invalid option n_gpu_layers: expected integer`.
- Negative integers and out-of-range values for `n_gpu_layers` are
  rejected: `invalid option n_gpu_layers: must be non-negative`.
- `pooling` accepts only the six strings listed in
  [Embeddings → Pooling](./embeddings.md#pooling).

## `Model::chat($prompt, ...)`

Named arguments — no array. PHP 8.0+ named-arguments syntax echoes the
ident verbatim, so you write `maxTokens: 256` (camelCase, per PSR-12).

| Argument      | Type                  | Default | See                                       |
| ------------- | --------------------- | ------- | ----------------------------------------- |
| `$prompt`     | `\Displace\Infer\Prompt` | required | [Prompts](./prompts.md)               |
| `maxTokens`   | `int`                 | `128`   | [Chat completions](./chat.md)             |
| `nCtx`        | `int`                 | `2048`  | [Chat completions](./chat.md)             |
| `temperature` | `float`               | `0.0`   | [Chat completions](./chat.md)             |
| `seed`        | `int`                 | `1234`  | [Chat completions](./chat.md)             |
| `options`     | `array`               | `[]`    | [Structured output](./structured-output.md) |

The trailing `options` array accepts (keys are mutually exclusive):

| Key       | Type            | Effect                                            |
| --------- | --------------- | ------------------------------------------------- |
| `grammar` | `string`        | GBNF grammar constraining every sampled token     |
| `schema`  | `array\|string` | JSON Schema (PHP array or JSON text) compiled to GBNF |

### Behavior

- `temperature = 0.0` is greedy (deterministic). `> 0.0` samples,
  controlled by `seed`.
- `seed` is only consulted when `temperature > 0`.
- `maxTokens` caps generation. Hitting it sets
  `Response::finishReason()` to `'length'`.
- `nCtx` is the *context window for this call*. If the rendered prompt
  exceeds it, `InferenceException` is raised before generation starts.

## `Model::raw($prompt, ...)`

Same named-argument shape as `chat()` plus `addBos`.

| Argument      | Type    | Default | See                                       |
| ------------- | ------- | ------- | ----------------------------------------- |
| `$prompt`     | `string` | required | [Raw completions](./raw.md)             |
| `maxTokens`   | `int`   | `128`   | [Chat completions](./chat.md)             |
| `nCtx`        | `int`   | `2048`  | [Chat completions](./chat.md)             |
| `temperature` | `float` | `0.0`   | [Chat completions](./chat.md)             |
| `seed`        | `int`   | `1234`  | [Chat completions](./chat.md)             |
| `addBos`      | `bool`  | `true`  | [Raw completions → addBos](./raw.md#addbos) |
| `options`     | `array` | `[]`    | Same `grammar`/`schema` keys as [`chat()`](#modelchatprompt-) |

## `Model::embed($text)`

Just the text. Pooling and embedding-mode are configured at load time
(see [`Model::load`](#modelloadpath-options) above).

| Argument | Type     | Default  | See                                |
| -------- | -------- | -------- | ---------------------------------- |
| `$text`  | `string` | required | [Embeddings](./embeddings.md)      |

## `Embedding` math

`Embedding` is read-only; the math methods return new instances rather
than mutating.

| Method                                 | Returns                  |
| -------------------------------------- | ------------------------ |
| `vector()`                             | `list<float>`            |
| `packed()`                             | `string` — little-endian float32, `pack('g*')`-identical |
| `dimensions()`                         | `int`                    |
| `norm()`                               | `float`                  |
| `normalize()`                          | new `Embedding`          |
| `cosineSimilarity(Embedding $other)`   | `float` (in `[-1, 1]`)   |

`cosineSimilarity` throws [`InferenceException`](../reference/exceptions.md)
on a dimension mismatch — see
[Embeddings → vector math](./embeddings.md#vector-math-built-in).

## `RerankModel::load($path, $options)`

Same array shape as `Model::load`, plus reranker-specific keys.

| Key            | Type     | Default | See                              |
| -------------- | -------- | ------- | -------------------------------- |
| `n_gpu_layers` | `int`    | `0`     | [Performance tuning](../advanced/performance.md) |
| `use_mmap`     | `bool`   | `true`  | [Performance tuning](../advanced/performance.md) |
| `use_mlock`    | `bool`   | `false` | [Performance tuning](../advanced/performance.md) |
| `n_ctx`        | `int`    | `4096`  | [Reranking → sizing](./reranking.md#sizing-and-budgets) |
| `instruction`  | `string` | model-card default | [Reranking → instruction](./reranking.md#the-instruction-option) |

## `RerankModel` methods

| Method                                      | Returns                  |
| ------------------------------------------- | ------------------------ |
| `score(string $query, string $document)`    | `float` in `(0, 1)`      |
| `rank(string $query, array $documents, ?int $topK = null)` | `list<array{index: int, score: float}>`, best-first |
| `close()`                                   | `void` (idempotent)      |

## `Prompt`

Static factories + immutable `with*` builders.

| Method                                   | Returns                  |
| ---------------------------------------- | ------------------------ |
| `Prompt::system($content)`               | new `Prompt`             |
| `Prompt::user($content)`                 | new `Prompt`             |
| `withSystem($content)`                   | new `Prompt`             |
| `withUser($content)`                     | new `Prompt`             |
| `withAssistant($content)`                | new `Prompt`             |
| `messages()`                             | `list<Message>`          |
| `lastRole()`                             | `?string`                |
| `count()`                                | `int`                    |
| `isEmpty()`                              | `bool`                   |

See [Prompts](./prompts.md) for the immutability semantics.

## `Response`

Read-only. Six getters.

| Method                                   | Returns                  |
| ---------------------------------------- | ------------------------ |
| `text()`                                 | `string`                 |
| `reasoning()`                            | `?string`                |
| `answer()`                               | `string`                 |
| `hasReasoning()`                         | `bool`                   |
| `finishReason()`                         | `string` — `'eos'`/`'length'`/`'stop'` |
| `tokensGenerated()`                      | `int`                    |

See [Chat completions → Inspecting a Response](./chat.md#inspecting-a-response).

## Environment

Not strictly an option, but bears mentioning here:

| Variable          | Effect                                                              |
| ----------------- | ------------------------------------------------------------------- |
| `EXT_INFER_LOG=1` | Restore llama.cpp's verbose stderr logging (silenced by default).   |

See [Environment variables](../reference/environment.md).
