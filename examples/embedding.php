<?php

/*
 * examples/embedding.php — vector embeddings + cosine similarity.
 *
 * What you'll learn:
 *
 *     - how to load a model in embedding mode
 *     - how to extract a vector for a single text
 *     - how to compare two vectors with cosine similarity
 *
 * Run it like so:
 *
 *     php -d extension=$(pwd)/target/debug/libinfer.dylib \
 *         examples/embedding.php models/some-model.gguf
 *
 * Any GGUF can be loaded in embedding mode — the model just hands you back
 * its hidden state pooled across the input tokens. For real semantic-search
 * quality, reach for a purpose-built embedding model: BGE-small, E5-small,
 * GTE-small, Qwen3-Embedding, etc. The chat-tuned models in your downloads
 * folder will produce *a* vector, but not always a useful one — they were
 * never optimized for it.
 *
 * Pooling defaults to whatever the GGUF metadata declares. Override at load
 * time via the `pooling` option if a model ships without it or you want to
 * experiment:
 *
 *     Model::load($path, [
 *         'embedding' => true,
 *         'pooling'   => 'mean',  // or 'cls' | 'last' | 'rank' | 'none' | 'unspecified'
 *     ]);
 */

declare(strict_types=1);

use Displace\Infer\Embedding;
use Displace\Infer\InferException;
use Displace\Infer\Model;

if (!extension_loaded('infer')) {
    fwrite(STDERR, "ext-infer is not loaded.\n");
    exit(1);
}

$modelPath = $argv[1] ?? null;
if ($modelPath === null) {
    fwrite(STDERR, "usage: php examples/embedding.php <path/to/model.gguf>\n");
    exit(2);
}
if (!is_file($modelPath)) {
    fwrite(STDERR, "no such file: {$modelPath}\n");
    exit(2);
}

try {
    // -------------------------------------------------------------------
    // 1. Load in embedding mode
    // -------------------------------------------------------------------
    //
    // `embedding: true` flips the handle so `Model::embed()` is allowed.
    // The flag exists to make the intent explicit at load time: without it,
    // `embed()` throws with a helpful error instead of silently producing
    // a garbage vector from a generation context.
    //
    // `chat()` and `raw()` still work on this handle — they build their own
    // generation context per call.
    $model = Model::load($modelPath, ['embedding' => true]);

    // -------------------------------------------------------------------
    // 2. Embed a batch of sentences
    // -------------------------------------------------------------------
    //
    // Cosine similarity is the standard "semantically close?" metric for
    // sentence embeddings. It compares the *direction* of two vectors and
    // ignores their magnitude, so it's customary to normalize each
    // embedding to unit length first. `Embedding::normalize()` returns a
    // new Embedding without mutating the original.
    $sentences = [
        'The cat sat on the mat.',
        'A feline rested on the rug.',
        'I went grocery shopping yesterday.',
    ];
    $embeddings = array_map(
        fn(string $s): Embedding => $model->embed($s)->normalize(),
        $sentences,
    );

    printf("dimensions: %d\n\n", $embeddings[0]->dimensions());

    // -------------------------------------------------------------------
    // 3. Print pairwise cosine similarity
    // -------------------------------------------------------------------
    //
    // For a real semantic-search use case you'd index the vectors into
    // something like a `pgvector` column or a flat file you mmap, and run
    // top-k nearest-neighbor queries. This is the same primitive, just
    // brute-forced for illustration.
    //
    // What good output looks like with a purpose-built embedding model:
    //
    //     sim(0, 1) ≈ +0.85  (paraphrase — cat-mat ≈ feline-rug)
    //     sim(0, 2) ≈ +0.10  (unrelated)
    //     sim(1, 2) ≈ +0.10  (unrelated)
    //
    // A chat-tuned model in embedding mode will have noisier numbers but
    // the ordering usually still holds for short, distinctive sentences.
    foreach ($embeddings as $i => $a) {
        foreach ($embeddings as $j => $b) {
            if ($j <= $i) {
                continue;
            }
            printf(
                "sim(%d, %d) = %+.4f  | %s  <->  %s\n",
                $i,
                $j,
                $a->cosineSimilarity($b),
                $sentences[$i],
                $sentences[$j],
            );
        }
    }

    $model->close();
} catch (InferException $e) {
    fwrite(STDERR, get_class($e) . ': ' . $e->getMessage() . PHP_EOL);
    exit(1);
}
