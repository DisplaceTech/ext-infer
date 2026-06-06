--TEST--
Model::close() is idempotent
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
$model->close();
$model->close();
echo "OK\n";
?>
--EXPECT--
OK
