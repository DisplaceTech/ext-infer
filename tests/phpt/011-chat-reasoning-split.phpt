--TEST--
Response splits reasoning from answer for Qwen3-class models
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
// This test asserts the Response-level reasoning/answer split. It is
// strongest on a reasoning model that uses <think>...</think> when invoked
// through its chat template (Qwen3, DeepSeek R1, ...). For non-reasoning
// models the EXPECT block degrades gracefully:
//   - text == answer            ✓
//   - reasoning is null         ✓
//   - hasReasoning() is false   ✓

use Displace\Infer\Model;
use Displace\Infer\Prompt;

$model = Model::load(getenv('INFER_TEST_MODEL'));
$prompt = Prompt::user('What is 2+2?');
$resp = $model->chat($prompt, maxTokens: 512, temperature: 0.0);

$text       = $resp->text();
$answer     = $resp->answer();
$reasoning  = $resp->reasoning();
$hasReason  = $resp->hasReasoning();

// If reasoning was emitted, text must contain a <think> tag and answer must not.
echo "consistent: ", (function () use ($text, $answer, $reasoning, $hasReason) {
    if ($hasReason) {
        return str_contains($text, '<think>') && !str_contains($answer, '<think>');
    }
    return $reasoning === null && $answer === $text;
})() ? "yes" : "no", "\n";

// answer() must be non-empty whether or not reasoning was emitted.
echo "answer_non_empty: ", strlen($answer) > 0 ? "yes" : "no", "\n";

$model->close();
?>
--EXPECT--
consistent: yes
answer_non_empty: yes
