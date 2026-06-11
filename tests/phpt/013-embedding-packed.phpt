--TEST--
Embedding::packed() returns pack('g*')-identical little-endian float32 bytes
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

$path  = getenv('INFER_TEST_MODEL');
$model = Model::load($path, ['embedding' => true, 'pooling' => 'last']);

$emb    = $model->embed('The quick brown fox jumps over the lazy dog.');
$packed = $emb->packed();

echo "is_string: ", is_string($packed) ? "yes" : "no", "\n";
echo "length_is_4x_dimensions: ",
    strlen($packed) === 4 * $emb->dimensions() ? "yes" : "no",
    "\n";

// The contract: byte-identical to pack('g*') over the float array.
echo "matches_pack_g: ",
    $packed === pack('g*', ...$emb->vector()) ? "yes" : "no",
    "\n";

// Round-trips through unpack('g*') to the same float32 values.
$roundTrip = array_values(unpack('g*', $packed));
$matches   = count($roundTrip) === $emb->dimensions();
foreach ($emb->vector() as $i => $v) {
    // Both sides are f32-representable, so equality is exact.
    if ($roundTrip[$i] !== $v) {
        $matches = false;
        break;
    }
}
echo "unpack_round_trips: ", $matches ? "yes" : "no", "\n";

// normalize() then packed() — the embed→index handoff in one expression.
$unitPacked = $emb->normalize()->packed();
echo "normalized_packed_same_length: ",
    strlen($unitPacked) === strlen($packed) ? "yes" : "no",
    "\n";

$model->close();
?>
--EXPECT--
is_string: yes
length_is_4x_dimensions: yes
matches_pack_g: yes
unpack_round_trips: yes
normalized_packed_same_length: yes
