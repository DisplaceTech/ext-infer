--TEST--
Grammar-constrained generation: GBNF strings and JSON Schemas shape the output
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
use Displace\Infer\Model;
use Displace\Infer\Prompt;

$model = Model::load(getenv('INFER_TEST_MODEL'));

// 1. Raw GBNF: the output must be exactly one of the grammar's terminals,
//    no matter what the model would rather say.
$out = $model->raw(
    'Is the sky blue? Answer: ',
    maxTokens: 8,
    options: ['grammar' => 'root ::= "yes" | "no"'],
);
echo "gbnf_terminal: ", in_array($out, ['yes', 'no'], true) ? "yes" : "no ($out)", "\n";

// 2. JSON Schema as a PHP array: chat() must emit parseable JSON with
//    exactly the declared properties, in declaration order.
$response = $model->chat(
    Prompt::system('Extract the data. Output JSON only.')
        ->withUser('Maria is 31 years old and lives in Lisbon.'),
    maxTokens: 128,
    options: ['schema' => [
        'type' => 'object',
        'properties' => [
            'name' => ['type' => 'string'],
            'age'  => ['type' => 'integer'],
            'city' => ['type' => 'string'],
        ],
    ]],
);
$decoded = json_decode($response->answer(), true, flags: JSON_THROW_ON_ERROR);
echo "schema_keys: ", implode(',', array_keys($decoded)), "\n";
echo "schema_age_is_int: ", is_int($decoded['age']) ? "yes" : "no", "\n";

// 3. Same schema as a JSON string instead of an array.
$out = $model->raw(
    "Extract as JSON: Bob is 7.\n",
    maxTokens: 64,
    options: ['schema' => '{"type":"object","properties":{"name":{"type":"string"},"age":{"type":"integer"}}}'],
);
$decoded = json_decode($out, true, flags: JSON_THROW_ON_ERROR);
echo "schema_string_keys: ", implode(',', array_keys($decoded)), "\n";

// 4. Enum schema pins the value set.
$out = $model->raw(
    'Sentiment of "I love this!": ',
    maxTokens: 8,
    options: ['schema' => ['enum' => ['positive', 'negative', 'neutral']]],
);
echo "enum_member: ",
    in_array(json_decode($out), ['positive', 'negative', 'neutral'], true) ? "yes" : "no ($out)",
    "\n";

$model->close();
?>
--EXPECT--
gbnf_terminal: yes
schema_keys: name,age,city
schema_age_is_int: yes
schema_string_keys: name,age
enum_member: yes
