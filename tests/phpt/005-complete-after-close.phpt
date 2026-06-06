--TEST--
Model::complete() after close() throws InferenceException
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
$model->close();
try {
    $model->complete('hello');
    echo "FAIL: no exception\n";
} catch (\Displace\Infer\InferenceException $e) {
    echo "OK\n";
    echo ($e instanceof \Displace\Infer\InferException ? "yes\n" : "no\n");
}
?>
--EXPECT--
OK
yes
