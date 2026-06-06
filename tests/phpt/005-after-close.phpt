--TEST--
chat() and raw() both throw InferenceException after the model is closed
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
use Displace\Infer\InferenceException;
use Displace\Infer\Model;
use Displace\Infer\Prompt;

$model = Model::load(getenv('INFER_TEST_MODEL'));
$model->close();

try {
    $model->chat(Prompt::user('hi'));
    echo "FAIL_chat\n";
} catch (InferenceException $e) {
    echo "chat_after_close: yes\n";
}

try {
    $model->raw('hi');
    echo "FAIL_raw\n";
} catch (InferenceException $e) {
    echo "raw_after_close: yes\n";
}
?>
--EXPECT--
chat_after_close: yes
raw_after_close: yes
