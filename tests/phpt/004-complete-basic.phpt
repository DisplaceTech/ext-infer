--TEST--
Model::complete() returns a non-empty string for a basic prompt
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
$model = \Displace\Infer\Model::load(getenv('INFER_TEST_MODEL'));
$out = $model->complete('The capital of France is', [
    'max_tokens'  => 8,
    'temperature' => 0.0,
    'seed'        => 1,
]);
echo is_string($out) ? "string\n" : "not_string\n";
echo (strlen($out) > 0) ? "non_empty\n" : "empty\n";
$model->close();
?>
--EXPECT--
string
non_empty
