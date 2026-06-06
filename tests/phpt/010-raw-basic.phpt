--TEST--
Model::raw() returns a non-empty string for a bare prompt
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

$text = $model->raw('The capital of France is', maxTokens: 8, temperature: 0.0);
echo "is_string: ", is_string($text) ? "yes" : "no", "\n";
echo "non_empty: ", strlen($text) > 0 ? "yes" : "no", "\n";

$model->close();
?>
--EXPECT--
is_string: yes
non_empty: yes
