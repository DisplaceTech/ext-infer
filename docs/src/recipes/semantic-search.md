# Semantic search

Embed a corpus once, embed user queries on demand, return the closest
matches by cosine similarity. The foundation of every "search by
meaning, not keywords" pipeline.

## Minimal in-memory version

```php
use Displace\Infer\Model;

$model = Model::load('models/Qwen3-Embedding-0.6B-Q8_0.gguf', [
    'embedding' => true,
]);

// Embed the corpus once. In real code, do this offline and cache.
$corpus = [
    'doc-1' => 'PHP is a server-side scripting language.',
    'doc-2' => 'Cats are popular pets known for their independence.',
    'doc-3' => 'Rust provides memory safety without garbage collection.',
    'doc-4' => 'Dogs are descendants of wolves, domesticated millennia ago.',
];
$index = [];
foreach ($corpus as $id => $text) {
    // Normalize once so the search loop is a plain dot product.
    $index[$id] = $model->embed($text)->normalize();
}

// Search.
function search(Model $model, array $index, string $query, int $k = 3): array
{
    $q = $model->embed($query)->normalize();
    $hits = [];
    foreach ($index as $id => $emb) {
        $hits[$id] = $q->cosineSimilarity($emb);
    }
    arsort($hits);
    return array_slice($hits, 0, $k, preserve_keys: true);
}

print_r(search($model, $index, 'a typesafe language'));
// Array
// (
//     [doc-3] => 0.7421
//     [doc-1] => 0.4567
//     [doc-2] => 0.1234
// )

$model->close();
```

## Three things to know

### Normalize when you index

`Embedding::normalize()` returns a unit vector. With both sides
normalized, cosine similarity simplifies to a dot product:

```text
cos(a, b) = (a · b) / (||a|| · ||b||)
          = a_unit · b_unit            // if both are normalized
```

Normalize once at index time so the per-query work is just the dot
product. `Embedding::cosineSimilarity()` does the normalization
internally if you skip the explicit step — but you pay for it on
*every* call, which adds up across thousands of documents.

### Pick an embedding model, not a chat model

A chat-tuned model loaded with `'embedding' => true` will return *a*
vector, but the similarity numbers cluster too tightly to be useful at
scale. Use a purpose-built embedding model — see
[Choosing a model](../guide/models.md#embedding-small-fast).

What "useful" looks like with a real embedding model
(Qwen3-Embedding-0.6B):

```text
cat-mat ↔ feline-rug:      0.72   (paraphrase)
cat-mat ↔ grocery-shop:    0.29   (unrelated)
feline-rug ↔ grocery-shop: 0.26   (unrelated)
```

Same query with the chat-tuned Qwen3-0.6B (loaded in embedding mode):

```text
cat-mat ↔ feline-rug:      0.66
cat-mat ↔ grocery-shop:    0.51
feline-rug ↔ grocery-shop: 0.50
```

The chat model preserves the ordering — the related pair scores
highest — but the *gap* is much narrower, so the cut-off threshold
between "match" and "not match" is harder to draw.

### Cache the index

In production, the in-memory dictionary in the example above doesn't
scale past a few thousand documents — the search loop is O(corpus
size). Three upgrade paths:

- **Stay in-process with
  [ext-turbovec](https://github.com/DisplaceTech/ext-turbovec)** —
  a quantized, SIMD-accelerated index from the same stack. 100K
  1024-dim documents fit in ~50MB resident, persist with
  `write()`/`load()`, and search in microseconds. The natural next
  step when this recipe outgrows the PHP loop.
- **Persist embeddings to disk** (a JSON file, SQLite blob column).
  Saves the embed-time cost on subsequent runs but keeps the O(n)
  scan.
- **Index with a vector database**: `pgvector` (PostgreSQL extension),
  `sqlite-vec`, MySQL 9 `VECTOR`. Right when the database must remain
  the system of record for vectors too.

See [Semantic search with ext-infer](https://turbovec.displace.tech/recipes/semantic-search-with-ext-infer.html)
for the ext-turbovec pairing, or [RAG over markdown](./rag-with-php.md)
for a worked example using `sqlite-vec`.

## Re-ranking with a chat model

For higher-quality top-K, embed-rank-then-rerank-with-a-chat-model is
the canonical pattern:

```php
// 1. Coarse retrieval — embedding similarity, top 20.
$hits = search($embedModel, $index, $query, k: 20);

// 2. Fine reranking — ask a chat model to score each candidate.
$prompt = Prompt::system(
    'You are a relevance judge. Given a query and a document, ' .
    'respond with a single number between 0 and 1 indicating ' .
    'how relevant the document is to the query.'
);
$rerank = [];
foreach (array_keys($hits) as $docId) {
    $r = $chatModel->chat(
        $prompt->withUser("Query: {$query}\n\nDocument: {$corpus[$docId]}"),
        maxTokens: 8,
        temperature: 0.0,
    );
    $rerank[$docId] = (float) trim($r->answer());
}
arsort($rerank);
```

That's two model loads — one embedding, one chat. Reuse handles across
requests; loading is the expensive step.

## Next

- [RAG over markdown](./rag-with-php.md) — semantic search feeding
  into `Model::chat()`.
- [Embeddings guide](../guide/embeddings.md) — the underlying API.
- [Choosing a model](../guide/models.md) — picking an embedding model.
