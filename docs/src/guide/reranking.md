# Reranking

Embedding similarity is fast but coarse: it compares a query against
each document *separately*, through the bottleneck of two pooled
vectors. A reranker reads the query and a candidate document
**together** and scores their actual relevance — far more accurate,
far more expensive. The canonical two-stage pipeline plays them to
their strengths:

1. **Recall** — vector search over the whole corpus returns ~20–50
   candidates in microseconds.
2. **Precision** — the reranker scores only that short list and
   reorders it.

## RerankModel

`RerankModel` targets the
[Qwen3-Reranker GGUF family](https://huggingface.co/ggml-org/Qwen3-Reranker-0.6B-Q8_0-GGUF)
(0.6B / 4B / 8B):

```php
use Displace\Infer\RerankModel;

$reranker = RerankModel::load('models/Qwen3-Reranker-0.6B-Q8_0.gguf');

// Score one pair — a calibrated probability in (0, 1).
$score = $reranker->score(
    'how do I reset my password?',
    'To reset your password, open Settings > Security > Reset password.',
);  // ≈ 0.999

// Rank a candidate list — best-first ['index' => int, 'score' => float]
// rows, where index points back into your input array.
$rows = $reranker->rank($query, $candidateTexts, topK: 5);

foreach ($rows as $row) {
    printf("%.3f  %s\n", $row['score'], $candidateTexts[$row['index']]);
}
```

`rank()`'s row shape is deliberately identical to
[`Displace\AI\Contracts\Reranker::rerank()`](https://github.com/DisplaceTech/ai-contracts),
so wrapping it in the framework-facing contract is a pass-through.

## How scoring works

Qwen3-Reranker is a causal LM fine-tuned to answer a fixed yes/no
judgment prompt. ext-infer renders the model card's template around
each (query, document) pair, decodes it, and reads the logits of the
single next token: the score is the binary softmax
`P("yes") / (P("yes") + P("no"))`.

Because the score is a calibrated probability rather than an arbitrary
similarity, thresholding works: "drop everything under 0.3" is a
meaningful, corpus-independent filter — something cosine scores can't
give you.

## The instruction option

The judgment prompt embeds a task instruction. The default is the
generic one the model trains against ("Given a web search query,
retrieve relevant passages that answer the query"); tailoring it to
your corpus measurably improves separation:

```php
$reranker = RerankModel::load($path, [
    'instruction' => 'Given a customer support question, retrieve KB articles that resolve it',
]);
```

## Sizing and budgets

- Every `score()` / `rank()` pair renders template + query + document
  and must fit in `n_ctx` (default 4096; raise at load time for long
  documents, or chunk them — the same chunking you indexed with).
- Cost scales linearly with candidates: reranking 20 candidates is
  ~20 forward passes. Keep stage-1 `k` in the tens, not hundreds.
- The 0.6B reranker is the sweet spot for CPU latency; the same code
  loads the 4B/8B GGUFs when accuracy is worth the milliseconds.

## A complete two-stage pipeline

```php
// Stage 1: recall — ext-turbovec over packed embeddings.
$candidateRows = $index->search($embedder->embed($query)->packed(), k: 20);
$candidates = array_map(fn (array $r): string => $texts[$r['id']], iterator_to_array($candidateRows));

// Stage 2: precision — rerank the short list, keep the best 5.
$best = $reranker->rank($query, $candidates, topK: 5);
```

## Errors

| Condition | Exception |
| --- | --- |
| Model file missing/unreadable | `ModelLoadException` |
| Vocabulary can't express single-token yes/no | `ModelLoadException` (not a Qwen3-Reranker-family GGUF) |
| Pair overflows `n_ctx` | `InferenceException`, suggests raising `n_ctx` or chunking |
| `topK < 1` | `InferException` |
| Use after `close()` | `InferenceException` |
