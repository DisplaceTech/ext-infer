--TEST--
RerankModel scores and ranks documents by relevance (Qwen3-Reranker path)
--SKIPIF--
<?php
if (!extension_loaded('infer')) {
    echo 'skip ext-infer not loaded';
    exit;
}
$path = getenv('INFER_TEST_RERANK_MODEL');
if (!$path || !is_file($path)) {
    echo 'skip INFER_TEST_RERANK_MODEL not set to an existing Qwen3-Reranker GGUF';
}
?>
--FILE--
<?php
use Displace\Infer\InferenceException;
use Displace\Infer\InferException;
use Displace\Infer\RerankModel;

$reranker = RerankModel::load(getenv('INFER_TEST_RERANK_MODEL'));

$query = 'how do I reset my password?';
$docs  = [
    'The cafeteria menu rotates weekly with vegetarian options.',          // 0: irrelevant
    'To reset your password, open Settings > Security > Reset password.', // 1: relevant
    'Our offices are closed on public holidays.',                         // 2: irrelevant
];

// score() is a calibrated probability: relevant ≫ irrelevant.
$relevant   = $reranker->score($query, $docs[1]);
$irrelevant = $reranker->score($query, $docs[0]);
echo "score_in_range: ", ($relevant > 0.0 && $relevant < 1.0) ? "yes" : "no", "\n";
echo "relevant_wins: ", $relevant > $irrelevant + 0.5 ? "yes" : "no", "\n";

// rank() returns best-first {index, score} rows over the input order.
$rows = $reranker->rank($query, $docs);
echo "rank_count: ", count($rows), "\n";
echo "best_index: ", $rows[0]['index'], "\n";
echo "rows_sorted: ", $rows[0]['score'] >= $rows[1]['score'] && $rows[1]['score'] >= $rows[2]['score'] ? "yes" : "no", "\n";

// topK keeps only the best rows.
$top = $reranker->rank($query, $docs, topK: 1);
echo "topk_count: ", count($top), "\n";
echo "topk_best: ", $top[0]['index'], "\n";

// topK < 1 is refused.
try {
    $reranker->rank($query, $docs, topK: 0);
    echo "topk_zero: FAIL\n";
} catch (InferException $e) {
    echo "topk_zero_throws: yes\n";
}

// Empty candidate list is a no-op, not an error.
echo "empty_docs: ", $reranker->rank($query, []) === [] ? "yes" : "no", "\n";

// Closed handle refuses further work.
$reranker->close();
try {
    $reranker->score($query, 'anything');
    echo "closed: FAIL\n";
} catch (InferenceException $e) {
    echo "closed_throws: yes\n";
}

// Direct construction refused.
try {
    new RerankModel();
    echo "ctor: FAIL\n";
} catch (InferException $e) {
    echo "ctor_throws: yes\n";
}
?>
--EXPECT--
score_in_range: yes
relevant_wins: yes
rank_count: 3
best_index: 1
rows_sorted: yes
topk_count: 1
topk_best: 1
topk_zero_throws: yes
empty_docs: yes
closed_throws: yes
ctor_throws: yes
