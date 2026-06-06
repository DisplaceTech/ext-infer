# `ext-infer` examples

Self-contained scripts you can copy-paste from when wiring `ext-infer`
into a real project. Each one stays minimal: no framework overhead
beyond what its job actually requires.

| Example                             | What it shows                                                  | Deps              |
| ----------------------------------- | -------------------------------------------------------------- | ----------------- |
| [`hello-world.php`](hello-world.php) | One-shot `Model::chat()` round-trip with a system + user prompt. The shortest "is my install working?" script. | none              |
| [`embedding.php`](embedding.php)    | `Model::embed()` + `Embedding::cosineSimilarity()` for pairwise semantic similarity. Foundation of any RAG / semantic-search use case. | none              |
| [`chat-interactive/`](chat-interactive/) | Multi-turn console chat. A Symfony Console standalone app demonstrating immutable `Prompt` accumulation, reasoning-model handling, and graceful inference errors. | `symfony/console` |

## Picking a model

Most examples work against any GGUF that fits in memory. Two small
choices we've tested locally:

```sh
mkdir -p ../models  # project-root/models/, gitignored

# Qwen3-0.6B-Q8_0 — ~640 MB, Apache-2.0, chat-tuned reasoning model.
# Good default for hello-world.php and chat-interactive/.
curl -L -o ../models/Qwen3-0.6B-Q8_0.gguf \
    https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q8_0.gguf

# Qwen3-Embedding-0.6B-Q8_0 — ~640 MB, Apache-2.0, purpose-built
# embedding model. Use with embedding.php for realistic semantic-
# similarity numbers.
curl -L -o ../models/Qwen3-Embedding-0.6B-Q8_0.gguf \
    https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF/resolve/main/Qwen3-Embedding-0.6B-Q8_0.gguf
```

The embedding example will run against a chat-tuned model too (it'll
return a vector — just a noisier one). For RAG / semantic search /
anything where similarity scores need to be reliable, pick a
purpose-built embedding model.

## Running

If you've installed `ext-infer` system-wide (via `make install` or
`pie install` once we ship binaries), nothing extra is needed:

```sh
php examples/hello-world.php models/Qwen3-0.6B-Q8_0.gguf
```

If you're running against a development build instead, point at the
freshly built `.so`/`.dylib`:

```sh
php -d extension=$(pwd)/target/debug/libinfer.dylib \
    examples/hello-world.php models/Qwen3-0.6B-Q8_0.gguf
```

Substitute `.so` for `.dylib` on Linux.

## Silencing llama.cpp logs

`ext-infer` mutes llama.cpp's stderr by default — the model-layout /
KV-cache-sizing chatter is useful for debugging the engine but not for
running an app. Set `EXT_INFER_LOG=1` to bring it back when you want
to see what's happening under the hood:

```sh
EXT_INFER_LOG=1 php examples/hello-world.php models/qwen3.gguf
```

## What good looks like

`hello-world.php` against Qwen3-0.6B on a recent M-series:

```
$ php -d extension=… examples/hello-world.php models/Qwen3-0.6B-Q8_0.gguf
2 + 2 equals 4.
```

`embedding.php` against the dedicated embedding model:

```
$ php -d extension=… examples/embedding.php models/Qwen3-Embedding-0.6B-Q8_0.gguf
dimensions: 1024

sim(0, 1) = +0.7207  | The cat sat on the mat.  <->  A feline rested on the rug.
sim(0, 2) = +0.2865  | The cat sat on the mat.  <->  I went grocery shopping yesterday.
sim(1, 2) = +0.2561  | A feline rested on the rug.  <->  I went grocery shopping yesterday.
```

Paraphrase pair scores 0.72; unrelated pairs hover around 0.27. That
~0.45-point gap is what makes vectors usable for nearest-neighbor
search. Run the same example against the chat-tuned `Qwen3-0.6B-Q8_0`
and you'll see all three pairs land in the 0.50–0.66 range — the
ordering is right but the gap is much narrower, which is why
purpose-built embedding models matter for real semantic-search work.
