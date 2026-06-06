# Quick start

This page assumes you've already [installed](./installation.md) the
extension. From a cold install to a working answer in under a minute:

## 1. Grab a model

GGUF files are big. Even the smallest interesting ones are 600 MB
quantized. For getting started, [Qwen3-0.6B-Q8_0](https://huggingface.co/Qwen/Qwen3-0.6B-GGUF)
is a good first model — Apache-2.0 licensed, ~640 MB, fast on CPU,
good enough at toy questions:

```sh
mkdir -p models
curl -L -o models/Qwen3-0.6B-Q8_0.gguf \
    https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q8_0.gguf
```

See [Choosing a model](../guide/models.md) for the broader landscape.

## 2. Write the script

Save the following as `hello.php`:

```php
<?php

declare(strict_types=1);

use Displace\Infer\Model;
use Displace\Infer\Prompt;

$model = Model::load('models/Qwen3-0.6B-Q8_0.gguf');

$response = $model->chat(
    Prompt::system('You are a helpful, concise assistant.')
        ->withUser('What is 2+2?'),
    maxTokens: 256,
    temperature: 0.0,
);

echo $response->answer(), PHP_EOL;

$model->close();
```

Three things going on:

- `Model::load(...)` reads the GGUF into memory. Loading is the slow
  step — for a real app, load once and keep the handle around. See
  [Choosing a model](../guide/models.md).
- `Prompt::system(...)->withUser(...)` builds a chat prompt without
  any template tokens. The `Prompt` is immutable; each `with*` returns
  a new instance. See [Prompts](../guide/prompts.md).
- `$model->chat($prompt, ...)` renders the prompt through whatever
  chat template the GGUF ships, runs inference, and returns a
  [`Response`](../guide/chat.md). `answer()` is the model's reply
  with any `<think>...</think>` reasoning stripped.

## 3. Run it

If you installed via PIE (or `make install`), just:

```sh
php hello.php
```

If you're running against a `make build` artifact instead:

```sh
php -d extension=$(pwd)/target/debug/libinfer.dylib hello.php
```

Substitute `.so` on Linux. Expected output:

```text
2 + 2 equals 4.
```

## 4. What just happened

llama.cpp normally spams several hundred lines to stderr per inference
(model layout, KV-cache sizing, graph reservation). `ext-infer`
silences that by default — it's noise inside a PHP request and tends
to poison structured logs. Bring it back when you need to debug:

```sh
EXT_INFER_LOG=1 php hello.php
```

See [Environment variables](../reference/environment.md) for the
complete list.

## Next steps

- **[Verifying your install](./verifying.md)** — the canonical
  diagnostic checklist if the script above doesn't work.
- **[Prompts](../guide/prompts.md)** — multi-turn chat, system
  messages, immutability semantics.
- **[Embeddings](../guide/embeddings.md)** — `Model::embed()` plus
  cosine similarity.
- **[Multi-turn chat recipe](../recipes/multi-turn-chat.md)** — a
  ready-to-lift implementation of conversational state.
- **[`examples/chat-interactive/`](https://github.com/DisplaceTech/ext-infer/tree/main/examples/chat-interactive)**
  — a Symfony Console standalone app that takes the above further.
