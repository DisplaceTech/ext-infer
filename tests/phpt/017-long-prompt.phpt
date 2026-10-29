--TEST--
Prompts longer than llama.cpp's default 2048-token batch decode instead of aborting the process
--SKIPIF--
<?php
if (!extension_loaded('infer')) {
    echo 'skip ext-infer not loaded';
    exit;
}
$path = getenv('INFER_TEST_MODEL');
if (!$path || !is_file($path)) {
    echo 'skip INFER_TEST_MODEL not set to an existing GGUF file';
}
?>
--FILE--
<?php
use Displace\Infer\InferenceException;

$model = \Displace\Infer\Model::load(getenv('INFER_TEST_MODEL'));

// ~2,250 tokens: just past llama.cpp's 2048 default n_batch (kept close to
// the boundary so the debug-build decode fits run-tests' timeout). Before
// 0.2.1 this tripped GGML_ASSERT(n_tokens_all <= cparams.n_batch) and the
// whole PHP process aborted — no exception, no shutdown functions.
$long = str_repeat('The archive stores sermons, broadcasts and lectures. ', 225) . "\nSummary:";
$text = $model->raw($long, maxTokens: 4, nCtx: 2560);
echo "decoded: ", is_string($text) ? "yes" : "no", "\n";

// Past nCtx stays a clean, catchable error.
try {
    $model->raw($long, maxTokens: 4, nCtx: 2048);
    echo "no exception\n";
} catch (InferenceException $e) {
    echo "too long: ", str_contains($e->getMessage(), 'n_ctx is only 2048') ? "caught" : $e->getMessage(), "\n";
}

$model->close();
?>
--EXPECT--
decoded: yes
too long: caught
