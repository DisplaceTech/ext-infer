--TEST--
Model::embed() returns an Embedding with vector math helpers
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
use Displace\Infer\Embedding;
use Displace\Infer\InferenceException;
use Displace\Infer\Model;

$path = getenv('INFER_TEST_MODEL');

// Loaded with embedding: true → embed() works.
$model = Model::load($path, ['embedding' => true, 'pooling' => 'last']);
$emb   = $model->embed('Hello world.');

echo "is_embedding: ", $emb instanceof Embedding ? "yes" : "no", "\n";
echo "dimensions_positive: ", $emb->dimensions() > 0 ? "yes" : "no", "\n";
echo "vector_count_matches_dimensions: ",
    count($emb->vector()) === $emb->dimensions() ? "yes" : "no",
    "\n";

// Self-cosine of a non-zero vector is exactly 1 within fp precision.
echo "cosine_self_one: ", round($emb->cosineSimilarity($emb), 4) === 1.0 ? "yes" : "no", "\n";

// normalize() returns a unit vector.
$unit = $emb->normalize();
echo "norm_after_normalize_one: ",
    round($unit->norm(), 4) === 1.0 ? "yes" : "no",
    "\n";

// Cross-text cosine is between -1 and 1.
$other = $model->embed('Bonjour le monde.');
$cs    = $emb->cosineSimilarity($other);
echo "cosine_in_range: ", ($cs >= -1.0 && $cs <= 1.0) ? "yes" : "no", "\n";

$model->close();

// A model loaded WITHOUT embedding: true must refuse embed().
$gen = Model::load($path);
try {
    $gen->embed('hi');
    echo "embed_without_flag: FAIL\n";
} catch (InferenceException $e) {
    echo "embed_without_flag_throws: yes\n";
}
$gen->close();

// new Embedding() refused.
try {
    new Embedding();
    echo "ctor: FAIL\n";
} catch (\Displace\Infer\InferException $e) {
    echo "ctor_throws: yes\n";
}
?>
--EXPECT--
is_embedding: yes
dimensions_positive: yes
vector_count_matches_dimensions: yes
cosine_self_one: yes
norm_after_normalize_one: yes
cosine_in_range: yes
embed_without_flag_throws: yes
ctor_throws: yes
