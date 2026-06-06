<?php

/*
 * Minimal smoke test for ext-infer.
 *
 * Usage:
 *     php -d extension=/path/to/libinfer.{so,dylib} \
 *         examples/hello-world.php path/to/model.gguf [prompt]
 *
 * Defaults to "The capital of France is " when no prompt is given. The
 * extension silences llama.cpp's stderr chatter by default; set
 * `EXT_INFER_LOG=1` in the environment if you want to see it.
 */

declare(strict_types=1);

use Displace\Infer\InferException;
use Displace\Infer\Model;

if (!extension_loaded('infer')) {
    fwrite(STDERR, "ext-infer is not loaded. Build with `make build` and either run\n");
    fwrite(STDERR, "`make install`, or pass `-d extension=target/debug/libinfer.dylib`\n");
    fwrite(STDERR, "(or .so on Linux) on the php command line.\n");
    exit(1);
}

$modelPath = $argv[1] ?? null;
if ($modelPath === null) {
    fwrite(STDERR, "usage: php examples/hello-world.php <path/to/model.gguf> [prompt]\n");
    exit(2);
}
if (!is_file($modelPath)) {
    fwrite(STDERR, "no such file: {$modelPath}\n");
    exit(2);
}

$prompt = $argv[2] ?? 'The capital of France is';

try {
    $model = Model::load($modelPath);
    $reply = $model->complete($prompt, [
        'max_tokens'  => 32,
        'temperature' => 0.0,
    ]);
    $model->close();
} catch (InferException $e) {
    fwrite(STDERR, get_class($e) . ': ' . $e->getMessage() . PHP_EOL);
    exit(1);
}

echo $prompt, $reply, PHP_EOL;
