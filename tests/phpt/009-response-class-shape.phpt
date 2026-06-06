--TEST--
Response is registered but not directly constructable
--SKIPIF--
<?php
if (!extension_loaded('infer')) echo 'skip ext-infer not loaded';
?>
--FILE--
<?php
use Displace\Infer\InferException;
use Displace\Infer\Response;

echo "class_exists: ", class_exists(Response::class) ? "yes" : "no", "\n";

$rc = new \ReflectionClass(Response::class);
$methods = array_map(fn($m) => $m->getName(), $rc->getMethods(\ReflectionMethod::IS_PUBLIC));
sort($methods);
echo "methods: ", implode(',', $methods), "\n";

try { new Response(); echo "FAIL_ctor\n"; }
catch (InferException $e) {
    echo "ctor_throws: yes\n";
    echo "ctor_message_mentions_chat: ", (str_contains($e->getMessage(), 'Model::chat') ? "yes" : "no"), "\n";
}
?>
--EXPECT--
class_exists: yes
methods: __construct,answer,finishReason,hasReasoning,reasoning,text,tokensGenerated
ctor_throws: yes
ctor_message_mentions_chat: yes
