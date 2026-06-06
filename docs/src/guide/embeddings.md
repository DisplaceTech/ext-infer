# Embeddings

`Model::embed()` turns a piece of text into a fixed-length vector of
floats. Cosine similarity between two such vectors approximates
semantic similarity between the texts they came from — that's the
foundation of every semantic-search / RAG pipeline.

```php
public function embed(string $text): \Displace\Infer\Embedding;
```

## Enable embedding mode at load time

Embedding generation requires a context built with
`with_embeddings(true)` under the hood. Because that conflicts with
generation mode for a given context, `ext-infer` makes the choice
explicit *at load*:

```php
use Displace\Infer\Model;

$model = Model::load('models/embedding-model.gguf', [
    'embedding' => true,
]);
```

With `embedding: true`, `embed()` works. Without it, `embed()` throws:

```
InferenceException: Model::embed() requires loading with ['embedding' => true]
```

`chat()` and `raw()` still work on an embedding-loaded handle — they
build their own per-call context for generation. So one handle can do
both, but you opt in to `embed()` explicitly.

## Pooling

Sentence embeddings need a way to collapse the per-token hidden states
into a single vector. Different model families do this differently:

| Pooling          | Used by                                                   |
| ---------------- | --------------------------------------------------------- |
| `mean`           | BGE, GTE, E5 — average across tokens                      |
| `cls`            | original BERT — uses the `[CLS]` token's hidden state     |
| `last`           | Qwen3-Embedding — uses the last token's hidden state      |
| `rank`           | rerankers — emits a single score, not a vector            |
| `none`           | per-token vectors, no pooling                             |

Modern embedding GGUFs declare their pooling type in metadata.
`ext-infer`'s default is `'unspecified'` (trust the metadata):

```php
$model = Model::load($path, ['embedding' => true]);
// pooling: whatever the GGUF says (almost always correct)
```

Override if a GGUF ships without the metadata or you want to
experiment:

```php
$model = Model::load($path, [
    'embedding' => true,
    'pooling'   => 'mean',   // 'unspecified' | 'none' | 'mean' | 'cls' | 'last' | 'rank'
]);
```

An unknown pooling string is rejected at load time, not at first
`embed()` call:

```
InferException: invalid option pooling: expected one of
unspecified/none/mean/cls/last/rank, got "weighted"
```

## Generating embeddings

```php
$emb = $model->embed('The cat sat on the mat.');

$emb->vector();        // list<float> — length matches the model's n_embd
$emb->dimensions();    // int — same as count($emb->vector())
```

Vectors are returned as PHP arrays of floats (doubles); internally we
hold `Vec<f32>` and let ext-php-rs convert f32 → f64 at the boundary,
which is lossless.

## Vector math, built in

`Embedding` carries the math you need most of the time so you don't
have to write a `numpy`-equivalent in PHP:

```php
$emb->norm();              // float — L2 norm: sqrt(sum_i x_i^2)
$emb->normalize();         // new Embedding scaled to unit length
$a->cosineSimilarity($b);  // float in [-1, 1]
```

`normalize()` returns a new `Embedding` — the original is not modified.
This matters for caching: cache the normalized form once, then every
subsequent `cosineSimilarity` call is just a dot product.

`cosineSimilarity()` throws on a dimension mismatch:

```
InferenceException: cannot compare embeddings of different
dimensions: 1024 vs 384
```

That's deliberate — comparing across model families is almost always a
bug, and silently returning a number would hide it.

### Why normalize before comparing?

Cosine similarity ignores magnitude — it compares *direction*. If
either vector has magnitude zero, the answer is undefined; we return
`0.0` rather than NaN. If both are non-zero, `cosineSimilarity` does
the right thing on un-normalized vectors too. But:

- For a fixed corpus you query against, normalizing once is cheap and
  makes the inner loop a single dot product:
  `array_sum(array_map(fn($x, $y) => $x * $y, $a, $b))`.
- For `pgvector` / `sqlite-vec` storage, you usually want normalized
  vectors stored so the database can use the inner-product operator
  (`<#>` in pgvector) instead of the cosine operator (`<=>`).

A canonical pipeline:

```php
$query = $model->embed($userQuestion)->normalize();
$best  = null;
$bestScore = -INF;
foreach ($corpusEmbeddings as $docId => $docEmb) {
    // $docEmb is also pre-normalized
    $score = $query->cosineSimilarity($docEmb);
    if ($score > $bestScore) {
        $best = $docId;
        $bestScore = $score;
    }
}
```

For real-world indexing — even at a few thousand documents — push the
storage into a database. See [Semantic search](../recipes/semantic-search.md)
and [RAG over markdown](../recipes/rag-with-php.md).

## Choosing an embedding model

The chat-tuned models people download for completions (Qwen3-0.6B,
Llama 3.2 3B, Mistral 7B) can be loaded with `embedding: true` and will
return *a* vector — but it's not what they were trained for, and
similarity numbers are noisier than what a purpose-built embedding
model produces.

| Model family               | Dims | Notes                                            |
| -------------------------- | ---- | ------------------------------------------------ |
| **Qwen3-Embedding** (0.6B) | 1024 | Apache-2.0. Same architecture as Qwen3-0.6B, retrained for embeddings. Strong default. |
| **BGE-small / BGE-large**  | 384 / 1024 | Beijing Academy of AI. Widely used, mean pooling. |
| **E5-small / E5-large**    | 384 / 1024 | Microsoft. Trained on text similarity tasks.     |
| **GTE-small / GTE-large**  | 384 / 1024 | Alibaba.                                          |

See [Choosing a model](./models.md) for more on GGUF quants and what
size to start with.

## Next

- [Semantic search recipe](../recipes/semantic-search.md) — embed a
  corpus, query, sort by similarity.
- [RAG over markdown](../recipes/rag-with-php.md) — semantic search
  feeding into `Model::chat()`.
- [Choosing a model](./models.md) — chat vs embedding, sizes, formats.
