--TEST--
Model::load() throws ModelLoadException for a missing file
--SKIPIF--
<?php
if (!extension_loaded('infer')) echo 'skip ext-infer not loaded';
?>
--FILE--
<?php
try {
    \Displace\Infer\Model::load('/definitely/not/a/real/path.gguf');
    echo "FAIL: no exception\n";
} catch (\Displace\Infer\ModelLoadException $e) {
    echo "OK: ModelLoadException\n";
    echo "is_infer: " . ($e instanceof \Displace\Infer\InferException ? 'yes' : 'no') . "\n";
    echo "is_runtime: " . ($e instanceof \RuntimeException ? 'yes' : 'no') . "\n";
}
?>
--EXPECT--
OK: ModelLoadException
is_infer: yes
is_runtime: yes
