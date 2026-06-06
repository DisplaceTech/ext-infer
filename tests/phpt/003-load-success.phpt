--TEST--
Model::load() returns a Model instance for a real GGUF
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
echo ($model instanceof \Displace\Infer\Model) ? "OK\n" : "FAIL\n";
$model->close();
?>
--EXPECT--
OK
