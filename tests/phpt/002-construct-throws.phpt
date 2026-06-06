--TEST--
new Model() refuses direct construction
--SKIPIF--
<?php
if (!extension_loaded('infer')) echo 'skip ext-infer not loaded';
?>
--FILE--
<?php
try {
    new \Displace\Infer\Model();
    echo "FAIL: no exception\n";
} catch (\Displace\Infer\InferException $e) {
    echo "OK: " . $e->getMessage() . "\n";
}
?>
--EXPECTF--
OK: %sModel::load()%s
