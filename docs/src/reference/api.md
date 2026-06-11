# API surface

The complete public PHP API in one place. Every method, every
argument, every return type. Read this when you know what you're
looking for and just want the signature; read the [Guide](../guide/prompts.md)
when you want to understand *why*.

For an authoritative copy in PHP-stub form (consumed by IDEs and
static analyzers like PHPStan), see
[`stubs/infer.stubs.php`](https://github.com/DisplaceTech/ext-infer/blob/main/stubs/infer.stubs.php).

## `Displace\Infer\Model`

```php
final class Model
{
    public static function load(
        string $path,
        array  $options = [],
    ): self;

    public function chat(
        \Displace\Infer\Prompt $prompt,
        int   $maxTokens   = 128,
        int   $nCtx        = 2048,
        float $temperature = 0.0,
        int   $seed        = 1234,
        array $options     = [],   // ['grammar' => gbnf] xor ['schema' => jsonSchema]
    ): \Displace\Infer\Response;

    public function raw(
        string $prompt,
        int    $maxTokens   = 128,
        int    $nCtx        = 2048,
        float  $temperature = 0.0,
        int    $seed        = 1234,
        bool   $addBos      = true,
        array  $options     = [],  // same grammar/schema keys as chat()
    ): string;

    public function embed(
        string $text,
    ): \Displace\Infer\Embedding;

    public function close(): void;
}
```

`new Model()` throws — use `Model::load()`. `close()` is idempotent
(safe to call from `finally` blocks).

See [Choosing a model](../guide/models.md), [Chat completions](../guide/chat.md),
[Raw completions](../guide/raw.md),
[Structured output](../guide/structured-output.md),
[Embeddings](../guide/embeddings.md),
and [Options reference](../guide/options.md).

## `Displace\Infer\RerankModel`

```php
final class RerankModel
{
    public static function load(
        string $path,
        array  $options = [],  // n_gpu_layers, use_mmap, use_mlock, n_ctx, instruction
    ): self;

    public function score(string $query, string $document): float;  // (0, 1)

    /**
     * @param list<string> $documents
     * @return list<array{index: int, score: float}>  best-first
     */
    public function rank(string $query, array $documents, ?int $topK = null): array;

    public function close(): void;
}
```

`new RerankModel()` throws — use `RerankModel::load()`. Targets the
Qwen3-Reranker GGUF family; `rank()`'s rows are shaped like
`Displace\AI\Contracts\Reranker::rerank()`. See
[Reranking](../guide/reranking.md).

## `Displace\Infer\Prompt`

```php
final class Prompt
{
    public static function system(string $content): self;
    public static function user(string $content): self;

    public function withSystem(string $content): self;
    public function withUser(string $content): self;
    public function withAssistant(string $content): self;

    /** @return list<\Displace\Infer\Message> */
    public function messages(): array;

    public function lastRole(): ?string;
    public function count(): int;
    public function isEmpty(): bool;
}
```

Immutable. `new Prompt()` throws — use a factory. See
[Prompts](../guide/prompts.md).

## `Displace\Infer\Message`

```php
final class Message
{
    public function role(): string;    // 'system' | 'user' | 'assistant'
    public function content(): string;
}
```

Read-only. Constructed only by `Prompt`; `new Message()` throws.

## `Displace\Infer\Response`

```php
final class Response
{
    public function text(): string;
    public function reasoning(): ?string;
    public function answer(): string;
    public function hasReasoning(): bool;
    public function finishReason(): string;  // 'eos' | 'length' | 'stop'
    public function tokensGenerated(): int;
}
```

Read-only. Constructed only by `Model::chat()`; `new Response()`
throws. See [Chat completions](../guide/chat.md#inspecting-a-response).

## `Displace\Infer\Embedding`

```php
final class Embedding
{
    /** @return list<float> */
    public function vector(): array;

    public function packed(): string;  // little-endian float32, pack('g*')-identical

    public function dimensions(): int;
    public function norm(): float;
    public function normalize(): self;
    public function cosineSimilarity(\Displace\Infer\Embedding $other): float;
}
```

Read-only. Constructed only by `Model::embed()`; `new Embedding()`
throws. See [Embeddings](../guide/embeddings.md).

## Exception hierarchy

```php
\RuntimeException
└── Displace\Infer\InferException
    ├── Displace\Infer\ModelLoadException
    └── Displace\Infer\InferenceException
```

`InferException` extends PHP's built-in `\RuntimeException`, so any
generic `catch (\RuntimeException $e)` clause sees `ext-infer` errors.
See [Exceptions](./exceptions.md) for which methods raise which
subclass.

## Conventions

- **Direct construction is refused** on `Prompt`, `Message`, `Response`,
  `Embedding`, and `Model`. Each one throws `InferException` from its
  `__construct` with a hint at the right factory. This is so an
  arbitrary `new Embedding()` can't lie about which model produced it.
- **All `with*` methods on `Prompt` return a new instance.** They
  never mutate. This is the only place the API exposes the "build by
  chaining" pattern; `Embedding::normalize()` also returns a new
  instance.
- **Sampling args are named, never positional.** `Model::chat()` and
  `Model::raw()` use PHP 8 named arguments
  (`maxTokens: 256, temperature: 0.7`). Load options — and the
  constraint options (`grammar`/`schema`) on `chat()`/`raw()` — *are*
  arrays because they're rare and compose with config-from-disk
  patterns.
