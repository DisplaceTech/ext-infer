# Choosing a model

`ext-infer` loads any [GGUF](https://github.com/ggerganov/ggml/blob/master/docs/gguf.md)
file llama.cpp can handle. Picking *which* GGUF is the most important
choice you'll make — it dominates inference quality, memory footprint,
and latency. This page is a tour of the landscape.

## What is GGUF?

GGUF (GPT-Generated Unified Format) is llama.cpp's native model
format. A `.gguf` file packs:

- **Weights** in a specific quantization.
- **Tokenizer** (vocabulary + merges).
- **Architecture metadata** (layer count, hidden size, attention
  config) so llama.cpp knows how to run the model without a separate
  config file.
- **Chat template** (for instruct models) so `Model::chat()` knows how
  to render messages.
- **Pooling type** (for embedding models) so `Model::embed()` knows how
  to collapse hidden states.

GGUF files self-contain everything `ext-infer` needs. There is no
separate config / tokenizer / vocab file to manage.

## Model families

There are three broad categories you'll encounter:

| Category    | What `ext-infer` method to use | Examples                                                       |
| ----------- | ------------------------------ | -------------------------------------------------------------- |
| **Base / pretrained** | [`raw()`](./raw.md) only | Llama 3 base, Mistral 7B base, Qwen base                       |
| **Chat / instruct**   | [`chat()`](./chat.md), `raw()` | Qwen3-Instruct, Llama 3.x Instruct, Mistral Instruct           |
| **Embedding / reranker** | [`embed()`](./embeddings.md) | Qwen3-Embedding, BGE, E5, GTE                                  |

A chat model loaded with `'embedding' => true` will return a vector,
but it's not what the model was optimized for — the vectors are
noisier than what a purpose-built embedding model produces. The
reverse (`chat()` against a pure embedding GGUF) usually fails because
embedding-only models don't ship a chat template.

## Quantization

A 7B-parameter model at full precision is ~14 GB on disk. Quantization
trades a small amount of quality for a much smaller, faster file. The
suffixes you'll see in GGUF filenames:

| Suffix      | Approx. size for a 7B model | Quality        | Notes                                                                 |
| ----------- | --------------------------- | -------------- | --------------------------------------------------------------------- |
| `F16`       | ~14 GB                      | Lossless       | Reference. Rarely worth the size unless you have plenty of memory.    |
| `Q8_0`      | ~7 GB                       | Near-lossless  | Good default when you can afford the disk.                            |
| `Q6_K`      | ~5.5 GB                     | Excellent      |                                                                       |
| `Q5_K_M`    | ~5 GB                       | Very good      |                                                                       |
| `Q4_K_M`    | ~4.5 GB                     | Good           | **The most popular size/quality compromise.**                         |
| `Q4_K_S`    | ~4 GB                       | Solid          |                                                                       |
| `Q3_K_M`    | ~3.5 GB                     | Noticeable degradation | Useful on memory-constrained boxes.                          |
| `Q2_K`      | ~2.5 GB                     | Significant degradation | Last resort.                                                  |

The K-family (`Q4_K_M`, etc.) uses *k-quants*, a smarter scheme than
the legacy non-K variants (`Q4_0`, `Q4_1`). Prefer K-quants when both
are offered for the same model.

### Picking a quant

Two questions:

1. **How much memory can you spend?** Quants below `Q4_K_M` save space
   at increasing quality cost. Above `Q4_K_M`, the marginal gain per
   GB shrinks fast.
2. **Is the model small enough that quantization barely matters?**
   For sub-1B models like Qwen3-0.6B, even `Q8_0` is ~640 MB —
   negligible by 2026 standards. Take the quality bump.

A good default rule: `Q4_K_M` for models > 3B, `Q8_0` for smaller
models.

## Recommended starting points

What we've actually tested against:

### Chat (smallest reasonable)

**`Qwen/Qwen3-0.6B-GGUF`** — Apache-2.0, 600M params, Q8 ≈ 640 MB.
Reasoning model: emits `<think>…</think>` blocks through its chat
template, which [`Response`](./chat.md#inspecting-a-response) splits
for you. Great for getting started; not great for production-quality
answers.

```sh
curl -L -o models/Qwen3-0.6B-Q8_0.gguf \
    https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q8_0.gguf
```

### Chat (production-ish)

**`bartowski/Qwen3-7B-Instruct-GGUF`** at `Q4_K_M` (~4.4 GB) — same
family, much better reasoning quality. Or **`bartowski/Llama-3.2-3B-Instruct-GGUF`**
at `Q4_K_M` (~1.9 GB) for a smaller, non-reasoning option.

### Embedding (small, fast)

**`Qwen/Qwen3-Embedding-0.6B-GGUF`** — Apache-2.0, 1024-dim
embeddings, `last` pooling baked into metadata. Same size as the chat
model; quality is competitive with BGE/E5 small variants.

```sh
curl -L -o models/Qwen3-Embedding-0.6B-Q8_0.gguf \
    https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF/resolve/main/Qwen3-Embedding-0.6B-Q8_0.gguf
```

Alternative: **`CompendiumLabs/bge-small-en-v1.5-gguf`** — 384-dim,
`mean` pooling, ~130 MB. Lower-quality vectors but tiny.

## Where to look for more

- **[Hugging Face GGUF tag](https://huggingface.co/models?library=gguf)**
  — `library=gguf` filters to GGUF-format models.
- **[bartowski](https://huggingface.co/bartowski)** — prolific publisher
  of quantized GGUFs for popular models. Reliable, consistent naming.
- **[mradermacher](https://huggingface.co/mradermacher)** — ditto.
- **The model's own official GGUF repo when one exists** (e.g.
  `Qwen/Qwen3-7B-Instruct-GGUF`) — always the most trusted source.

## License caveats

GGUF files inherit the underlying model's license. Some models that
are nominally "open" (Llama 3.x, Gemma) ship under custom licenses with
use restrictions; others (Qwen, Mistral, several smaller players) are
Apache-2.0 / MIT. Check the model card before depending on a model in
a commercial deployment.

`ext-infer` itself is MIT-licensed — the extension doesn't care which
GGUF you load, but downstream concerns are on you.

## Next

- [Embeddings](./embeddings.md) — when you've picked an embedding
  model.
- [Chat completions](./chat.md) — when you've picked a chat model.
- [Performance tuning](../advanced/performance.md) — `n_gpu_layers`,
  `mmap`, `mlock` for the model you ended up with.
