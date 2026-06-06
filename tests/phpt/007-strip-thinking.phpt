--TEST--
Model::complete() with strip_thinking=true removes <think>...</think> blocks
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
// Wrap the prompt in a Qwen3-style chat template so a reasoning model emits
// a `<think>...</think>` block. Non-reasoning models will simply not emit
// the tags; the strip is a no-op in that case, which is also a valid pass.
$prompt = "<|im_start|>user\nwhat is 2+2?<|im_end|>\n<|im_start|>assistant\n";

$model = \Displace\Infer\Model::load(getenv('INFER_TEST_MODEL'));

$raw     = $model->complete($prompt, ['max_tokens' => 200, 'temperature' => 0.0]);
$cleaned = $model->complete($prompt, ['max_tokens' => 200, 'temperature' => 0.0, 'strip_thinking' => true]);

// After stripping, neither tag may remain.
echo "no_open_after_strip: ",  (str_contains($cleaned, '<think>')  ? "no"  : "yes"), "\n";
echo "no_close_after_strip: ", (str_contains($cleaned, '</think>') ? "no"  : "yes"), "\n";

// The stripped output must not be longer than the raw output (it can only
// shrink — same generated text minus the tagged block).
echo "stripped_not_longer: ", (strlen($cleaned) <= strlen($raw) ? "yes" : "no"), "\n";

// The stripped output must be non-empty as long as the model produced any
// text outside the think block (Qwen3 always does, even with `/no_think`).
echo "non_empty: ", (strlen($cleaned) > 0 ? "yes" : "no"), "\n";

$model->close();
?>
--EXPECT--
no_open_after_strip: yes
no_close_after_strip: yes
stripped_not_longer: yes
non_empty: yes
