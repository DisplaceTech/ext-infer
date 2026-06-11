--TEST--
Grammar/schema option validation fails loudly and precisely
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
use Displace\Infer\InferException;
use Displace\Infer\Model;

$model = Model::load(getenv('INFER_TEST_MODEL'));

// grammar + schema together is ambiguous → refused.
try {
    $model->raw('x', options: ['grammar' => 'root ::= "a"', 'schema' => ['type' => 'string']]);
    echo "both: FAIL\n";
} catch (InferException $e) {
    echo "both_throws: ", str_contains($e->getMessage(), 'mutually exclusive') ? "yes" : "no", "\n";
}

// Unsupported schema keywords name themselves in the error instead of
// silently under-constraining.
try {
    $model->raw('x', options: ['schema' => ['type' => 'string', 'pattern' => '^a+$']]);
    echo "pattern: FAIL\n";
} catch (InferException $e) {
    echo "pattern_named: ", str_contains($e->getMessage(), 'pattern') ? "yes" : "no", "\n";
}

// Optional properties are out of the supported subset.
try {
    $model->raw('x', options: ['schema' => [
        'type' => 'object',
        'properties' => ['a' => ['type' => 'string'], 'b' => ['type' => 'string']],
        'required' => ['a'],
    ]]);
    echo "optional: FAIL\n";
} catch (InferException $e) {
    echo "optional_rejected: ", str_contains($e->getMessage(), 'optional properties') ? "yes" : "no", "\n";
}

// Malformed GBNF is rejected by llama.cpp's parser, surfaced as our exception.
try {
    $model->raw('x', options: ['grammar' => 'this is not ::= valid % gbnf ((']);
    echo "bad_gbnf: FAIL\n";
} catch (InferException $e) {
    echo "bad_gbnf_throws: yes\n";
}

// Schema must be an array or string.
try {
    $model->raw('x', options: ['schema' => 42]);
    echo "bad_type: FAIL\n";
} catch (InferException $e) {
    echo "bad_type_throws: yes\n";
}

$model->close();
?>
--EXPECT--
both_throws: yes
pattern_named: yes
optional_rejected: yes
bad_gbnf_throws: yes
bad_type_throws: yes
