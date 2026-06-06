<?php

/*
 * examples/hello-world.php — one-shot chat completion with ext-infer.
 *
 * The smallest interesting program you can write against ext-infer:
 *
 *     1. load a GGUF model from disk
 *     2. build a chat prompt (system + user)
 *     3. ask the model to answer
 *     4. print the answer
 *     5. release the model's memory
 *
 * Run it like so (replace the path with whatever GGUF you downloaded):
 *
 *     php -d extension=$(pwd)/target/debug/libinfer.dylib \
 *         examples/hello-world.php models/Qwen3-0.6B-Q8_0.gguf
 *
 * On Linux substitute `.so` for `.dylib`. On systems where you've already
 * run `make install`, drop the `-d extension=...` flag.
 *
 * Pass an extra argument to override the default question:
 *
 *     php -d extension=… examples/hello-world.php models/qwen3.gguf \
 *         "What's the highest mountain in Antarctica?"
 *
 * llama.cpp normally spams several hundred lines of stderr per inference
 * (model layout, KV-cache sizing, graph reservation, …). The extension
 * silences it by default; set EXT_INFER_LOG=1 in the environment to see
 * what's actually happening under the hood.
 */

declare(strict_types=1);

use Displace\Infer\InferException;
use Displace\Infer\Model;
use Displace\Infer\Prompt;

// ---------------------------------------------------------------------------
// 0. Sanity checks
// ---------------------------------------------------------------------------
//
// `extension_loaded()` confirms the .so/.dylib actually got picked up. The
// most common reason for it returning false is a missing or misspelled
// `-d extension=…` flag. The error message walks the user through the fix
// without making them go hunt for documentation.
if (!extension_loaded('infer')) {
    fwrite(STDERR, "ext-infer is not loaded. Either run `make install` (which\n");
    fwrite(STDERR, "wires it into your php.ini), or add\n");
    fwrite(STDERR, "    -d extension=\$(pwd)/target/debug/libinfer.dylib\n");
    fwrite(STDERR, "(.so on Linux) to your php command line.\n");
    exit(1);
}

$modelPath = $argv[1] ?? null;
if ($modelPath === null) {
    fwrite(STDERR, "usage: php examples/hello-world.php <path/to/model.gguf> [question]\n");
    exit(2);
}
if (!is_file($modelPath)) {
    fwrite(STDERR, "no such file: {$modelPath}\n");
    exit(2);
}

$question = $argv[2] ?? 'What is 2+2?';

try {
    // -------------------------------------------------------------------
    // 1. Load the model
    // -------------------------------------------------------------------
    //
    // `Model::load()` reads the GGUF into memory and stays loaded until you
    // call `close()` (or until the PHP process exits). Loading is the slow
    // part — for any non-trivial use you'll want to load once and reuse the
    // handle across many calls.
    //
    // The second argument is an associative array of load-time tuning
    // options. Common ones:
    //
    //   'n_gpu_layers' => int (default 0)   — offload N transformer layers
    //                                         to the GPU; 0 keeps everything
    //                                         on CPU. Combine with the
    //                                         `metal` cargo feature on
    //                                         macOS-arm64 for Apple GPU
    //                                         acceleration.
    //   'use_mmap'     => bool (default true) — memory-map the file. Almost
    //                                         always what you want.
    //   'use_mlock'    => bool (default false)— pin weights in RAM so the OS
    //                                         can't page them out. Useful
    //                                         on memory-constrained boxes
    //                                         where you'd rather OOM than
    //                                         thrash.
    $model = Model::load($modelPath);

    // -------------------------------------------------------------------
    // 2. Build the prompt
    // -------------------------------------------------------------------
    //
    // `Prompt` is the role-aware, immutable equivalent of "build a string
    // with <|im_start|> tokens". It mirrors `DateTimeImmutable`: each
    // `with*` returns a NEW prompt, so you can stash an intermediate value
    // and branch off it without worrying about shared mutable state.
    //
    // Start with either `Prompt::system($s)` or `Prompt::user($s)`. Here
    // we set a short system instruction to keep the answer concise.
    //
    // For a multi-turn conversation, keep appending: `->withUser(...)
    // ->withAssistant(...)`. See examples/chat-interactive/ for the full
    // shape.
    $prompt = Prompt::system('You are a helpful assistant. Answer in one short sentence.')
        ->withUser($question);

    // -------------------------------------------------------------------
    // 3. Ask the model
    // -------------------------------------------------------------------
    //
    // `Model::chat()` renders the prompt through whatever chat template
    // the model embeds in its GGUF (Qwen3 ships ChatML, Llama 3 ships its
    // own, etc.) and returns a `Response`.
    //
    // Named arguments cover sampling. The four most common:
    //
    //   maxTokens    int   default 128   — hard upper bound on generated
    //                                       tokens. Bump it if you're
    //                                       getting "length"-truncated
    //                                       answers.
    //   nCtx         int   default 2048  — context window for this call.
    //                                       Increase only if prompt + reply
    //                                       might exceed 2048 tokens.
    //   temperature  float default 0.0   — 0.0 is greedy (deterministic).
    //                                       Anything > 0 samples.
    //   seed         int   default 1234  — RNG seed for temperature > 0
    //                                       runs. Same seed + same prompt
    //                                       reproduces the same reply.
    $response = $model->chat($prompt, maxTokens: 256, temperature: 0.0);

    // `Response::answer()` returns the model's reply with any internal
    // `<think>...</think>` reasoning stripped. `Response::reasoning()` gives
    // you what was stripped (or `null` if the model wasn't a reasoning
    // model). `Response::text()` returns the full raw concatenation.
    $reply = $response->answer();

    // -------------------------------------------------------------------
    // 4. Release the model
    // -------------------------------------------------------------------
    //
    // `close()` drops the in-memory weights deterministically rather than
    // waiting for PHP's GC. It's a no-op if the model is already closed,
    // so it's safe to call from `finally` blocks.
    $model->close();
} catch (InferException $e) {
    // All exceptions thrown by ext-infer descend from
    // `Displace\Infer\InferException`, which itself extends
    // `\RuntimeException`. Catching InferException catches everything
    // (load failures, inference failures, bad options); catching the
    // subclasses lets you distinguish them when it matters.
    fwrite(STDERR, get_class($e) . ': ' . $e->getMessage() . PHP_EOL);
    exit(1);
}

echo $reply, PHP_EOL;
