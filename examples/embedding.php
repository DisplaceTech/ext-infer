<?php

/*
 * Embedding example for ext-infer.
 *
 * Demonstrates Model::embed() and the Embedding class — vector access,
 * dimensionality, L2 norm, normalization, and cosine similarity for
 * semantic-search-style comparisons.
 *
 * Usage:
 *     php -d extension=/path/to/libinfer.{so,dylib} \
 *         examples/embedding.php path/to/embedding-model.gguf
 *
 * Most reasoning/instruct GGUFs (Qwen3, Llama 3, ...) can be used in
 * embedding mode and will produce a hidden-state vector — quality and
 * dimensionality vary. For real semantic search, prefer a purpose-built
 * embedding model: BGE, E5, GTE, Qwen3-Embedding, etc.
 */

declare(strict_types=1);

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
    // `embedding: true` flips the handle into embedding mode. `pooling`
    // defaults to whatever the GGUF declares; we leave it unset here.
    $model = Model::load($modelPath, ['embedding' => true]);

    $sentences = [
        'The cat sat on the mat.',
        'A feline rested on the rug.',
        'I went grocery shopping yesterday.',
    ];

    // Embed each sentence and normalize for stable cosine similarity.
    $embeddings = array_map(
        fn(string $s) => $model->embed($s)->normalize(),
        $sentences,
    );

    printf("dimensions: %d\n\n", $embeddings[0]->dimensions());

    // Pairwise cosine similarity.
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
