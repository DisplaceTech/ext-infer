--TEST--
Model::chat() returns a Response with the model's answer
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
use Displace\Infer\Response;

$model = Model::load(getenv('INFER_TEST_MODEL'));

$prompt = Prompt::system('You are helpful. Answer in one short sentence.')
    ->withUser('What is 2+2?');

$response = $model->chat($prompt, maxTokens: 256, temperature: 0.0);

echo "is_response: ", $response instanceof Response ? "yes" : "no", "\n";
echo "answer_non_empty: ", strlen($response->answer()) > 0 ? "yes" : "no", "\n";
echo "tokens_positive: ", $response->tokensGenerated() > 0 ? "yes" : "no", "\n";
echo "finish_known: ", in_array($response->finishReason(), ['eos', 'length', 'stop'], true) ? "yes" : "no", "\n";

$model->close();
?>
--EXPECT--
is_response: yes
answer_non_empty: yes
tokens_positive: yes
finish_known: yes
