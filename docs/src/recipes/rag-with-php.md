# RAG over markdown

Retrieval-Augmented Generation: instead of asking the model what it
knows (and getting whatever its training data captured, possibly
incorrectly), embed your own documents into a vector store, retrieve
the most relevant ones at query time, and feed them to the model as
context. The model answers from your data.

This recipe walks through the smallest practical version: a folder of
markdown files, indexed once into `sqlite-vec`, queried on demand.

## Prerequisites

- An embedding model — [Qwen3-Embedding-0.6B](../guide/models.md#embedding-small-fast)
  works well.
- A chat model — Qwen3-7B-Instruct or similar.
- The [`sqlite-vec`](https://github.com/asg017/sqlite-vec) extension
  loaded into PHP's PDO SQLite (or use the `sqlite3` CLI tools).

```sh
# macOS — Homebrew has it
brew install asg017/sqlite-vec/sqlite-vec
# Linux — see the sqlite-vec README for distro packages
```

## Schema

A single table holds documents and their embeddings:

```sql
CREATE TABLE IF NOT EXISTS docs (
    id    INTEGER PRIMARY KEY AUTOINCREMENT,
    path  TEXT UNIQUE NOT NULL,
    body  TEXT NOT NULL
);

-- sqlite-vec virtual table for k-nearest-neighbor search.
-- 1024 dimensions matches Qwen3-Embedding-0.6B.
CREATE VIRTUAL TABLE IF NOT EXISTS doc_vecs USING vec0(
    id    INTEGER PRIMARY KEY,
    embed FLOAT[1024]
);
```

## Indexing

Walk a directory, embed each file, persist:

```php
declare(strict_types=1);

use Displace\Infer\Model;

$embedder = Model::load('models/Qwen3-Embedding-0.6B-Q8_0.gguf', [
    'embedding' => true,
]);

$pdo = new PDO('sqlite:rag.db');
$pdo->setAttribute(PDO::ATTR_ERRMODE, PDO::ERRMODE_EXCEPTION);
$pdo->sqliteCreateFunction('load_extension', 'sqlite_vec_init', 1);  // see sqlite-vec docs

$pdo->exec(file_get_contents('schema.sql'));

$insertDoc = $pdo->prepare(
    'INSERT INTO docs (path, body) VALUES (:path, :body)
     ON CONFLICT(path) DO UPDATE SET body = excluded.body
     RETURNING id'
);
$insertVec = $pdo->prepare(
    'INSERT OR REPLACE INTO doc_vecs (id, embed) VALUES (:id, :embed)'
);

$root = $argv[1] ?? './notes';
foreach (new RecursiveIteratorIterator(new RecursiveDirectoryIterator($root)) as $f) {
    if ($f->getExtension() !== 'md') {
        continue;
    }
    $body = file_get_contents($f->getPathname());
    $insertDoc->execute([':path' => $f->getPathname(), ':body' => $body]);
    $id = (int) $insertDoc->fetchColumn();

    // Pre-normalize so search is a dot product.
    $vector = $embedder->embed($body)->normalize()->vector();

    $insertVec->execute([
        ':id'    => $id,
        ':embed' => pack('f*', ...$vector),   // sqlite-vec wants float32 bytes
    ]);

    echo "indexed: {$f->getPathname()} ({$id})\n";
}

$embedder->close();
```

Run once to build the index, again whenever your notes change. For
larger corpora, chunk each file into ~500-token sections and embed
each chunk separately — sentence-level granularity gives better
retrieval than whole-file vectors.

## Retrieval + generation

```php
declare(strict_types=1);

use Displace\Infer\Model;
use Displace\Infer\Prompt;

$embedder = Model::load('models/Qwen3-Embedding-0.6B-Q8_0.gguf', [
    'embedding' => true,
]);
$chat = Model::load('models/Qwen3-7B-Instruct-Q4_K_M.gguf');

$pdo = new PDO('sqlite:rag.db');
$pdo->setAttribute(PDO::ATTR_ERRMODE, PDO::ERRMODE_EXCEPTION);

$query = $argv[1] ?? 'What did I decide about the migration?';

// 1. Embed the query.
$qvec = $embedder->embed($query)->normalize()->vector();

// 2. Top-k retrieval via sqlite-vec.
$stmt = $pdo->prepare(<<<SQL
    SELECT
        docs.path,
        docs.body,
        vec_distance_cosine(doc_vecs.embed, :qvec) AS distance
    FROM doc_vecs
    JOIN docs ON docs.id = doc_vecs.id
    ORDER BY distance ASC
    LIMIT 4
SQL);
$stmt->execute([':qvec' => pack('f*', ...$qvec)]);
$hits = $stmt->fetchAll(PDO::FETCH_ASSOC);

// 3. Build a context-injected prompt.
$context = '';
foreach ($hits as $i => $hit) {
    $context .= sprintf("--- Document %d (%s) ---\n%s\n\n", $i + 1, $hit['path'], $hit['body']);
}

$prompt = Prompt::system(<<<SYS
You answer questions strictly using the provided documents.
If the documents don't contain the answer, say so — do not invent.
Cite the document number when you quote.
SYS)
    ->withUser("Documents:\n\n{$context}\n\nQuestion: {$query}");

// 4. Ask.
$response = $chat->chat($prompt, maxTokens: 1024, temperature: 0.3);

echo $response->answer(), PHP_EOL;

$embedder->close();
$chat->close();
```

## What good output looks like

For a corpus of personal notes, a query like
*"what did I decide about the migration?"* returns:

```text
Based on Document 2 (notes/migration.md), you decided to defer the
schema change to Q3 in favor of shipping the redirect layer first.
The reasoning cited there was that the redirect layer was lower-risk
and would surface the migration's actual hot paths before you
committed to the column rename.
```

If the corpus doesn't contain the answer:

```text
The provided documents don't address the migration decision directly.
Document 1 mentions a planned schema change but doesn't record what
was decided. I'd need more context to answer.
```

That "I don't know" behavior is what the `system` prompt enforces. Models
will happily make up plausible answers without it.

## Knobs worth tuning

| Knob                                | Effect                                                  |
| ----------------------------------- | ------------------------------------------------------- |
| Top-k (the `LIMIT 4` above)         | More context = better answers but slower + risks the model conflating unrelated documents. 3–5 is a good default. |
| Chunk size at index time            | Whole-file is simple but coarse. 500-token chunks give finer retrieval at the cost of ~10x more vectors. |
| `temperature` on the chat model     | Set low (`0.0`–`0.3`) for factual answers; the model should be quoting, not improvising. |
| System prompt strictness            | "Cite documents" + "say so if unknown" is the difference between RAG and a model that just sometimes incorporates your context. |

## What this recipe doesn't cover

- **Reranking** — top-k by embedding similarity is fast but coarse.
  See the [Semantic search recipe](./semantic-search.md#re-ranking-with-a-chat-model)
  for the chat-model-as-reranker pattern.
- **Streaming responses** — `Model::chat()` is currently synchronous.
  See the [roadmap](https://github.com/DisplaceTech/ext-infer/blob/main/PLAN.md).
- **Production-grade chunking** — markdown-aware splitting that
  respects code blocks, headers, lists. Worth a library; not in scope
  for ext-infer itself.

## Next

- [Semantic search](./semantic-search.md) — the building-block
  underneath this.
- [Worker pools](./worker-pools.md) — running RAG queries under
  concurrent load.
- [Choosing a model](../guide/models.md) — picking the right
  embedding + chat models for your corpus.
