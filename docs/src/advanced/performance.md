# Performance tuning

A `Model::chat()` call has three dominant costs:

1. **Loading** — one-time on the first call. Dominated by disk I/O
   (or `mmap` setup) for the GGUF.
2. **Prompt prefill** — tokenize + forward-pass on the prompt. Scales
   roughly linearly with prompt length.
3. **Token generation** — sample, decode, sample, decode, … Scales
   linearly with `maxTokens` (or wherever the model chooses to stop).

This page walks through each, what knobs affect it, and what
trade-offs each knob carries.

## Reducing load time

Load is the slow part — for a 4 GB model from a cold cache, expect
1–3 seconds on SSD, longer on spinning rust.

### `use_mmap` (default `true`)

Memory-mapping the GGUF skips the explicit `read()` syscall and lets
the OS page weights in lazily. **Always leave this on** unless you're
diagnosing a specific mmap issue. Without it, load reads the entire
file upfront — slower for large models, identical for small ones once
cached.

```php
$model = Model::load($path, ['use_mmap' => true]);   // default
```

### `use_mlock` (default `false`)

`mlock` pins the model's pages in physical RAM so the OS can't page
them out. Useful when:

- You're on a memory-constrained machine and would rather OOM than
  thrash.
- You're serving a large model under unpredictable load and want
  predictable latency.

The cost: that physical memory is unavailable to anything else on the
system. Don't turn it on unless you know you want it.

```php
$model = Model::load($path, ['use_mlock' => true]);
```

On Linux, `mlock` has a per-process limit (`RLIMIT_MEMLOCK`). For
models larger than 64 MB (basically all of them), you'll need to raise
it via `/etc/security/limits.conf` or `ulimit -l unlimited`. macOS
doesn't enforce the same limit but may swap aggressively under
memory pressure.

### Sharing one load across many workers

If you're running multiple FPM workers, the OS automatically
deduplicates `mmap`'d pages across them. 16 workers loading the same
4 GB model consume ~4 GB of physical memory total, not 64. This is
why `use_mmap` matters even on machines with abundant RAM.

## Reducing prompt prefill cost

Prefill cost scales with the number of prompt tokens. The longest
prompts come from RAG pipelines that inject document context — see
[RAG over markdown](../recipes/rag-with-php.md).

### `nCtx` (default `2048`)

The context window for a single call. The rendered prompt + generated
tokens must fit. **Lower is faster** because llama.cpp allocates the
KV cache to `nCtx`, so a 32k context costs 16× more memory than a 2k
context even when most of it is unused.

```php
$model->chat($prompt, nCtx: 4096, maxTokens: 1024);
```

For typical RAG/chat use cases, `nCtx = 2048` to `4096` is plenty.
Go higher only when the model has been trained for it and you've
measured a quality benefit.

### Prompt length

The fastest prompt is a short prompt. Common ways to compress
without losing fidelity:

- **Drop boilerplate from system messages.** "You are a helpful
  assistant. Answer truthfully. Don't make things up. Be concise.
  Use markdown formatting. ..." is mostly cargo-culted. Test what's
  actually load-bearing.
- **Truncate conversation history.** Keep the last N turns rather
  than every turn since the dawn of the conversation. For most
  chatbots, N = 6–10 is plenty.
- **Summarize old turns.** Replace turns 1–50 with "Earlier, the
  user asked about X and you said Y." This is what production
  chatbots do above a certain length.

## Reducing token-generation cost

Once prefill is done, each generated token costs roughly the same.
Two knobs.

### `maxTokens` (default `128`)

The maximum number of generated tokens. Lower is faster. **The
default is conservative on purpose** — bump it for any non-trivial
generation:

```php
$model->chat($prompt, maxTokens: 512);   // ~4× the default budget
```

Set it high enough that legitimate answers complete, low enough that
runaway generations (which happen) don't wedge the worker for
minutes. For reasoning models, you'll want at least 512 — they spend
many tokens thinking.

When `finishReason() === 'length'`, you hit this budget. Surface it
to the caller so they can decide whether to bump or live with the
truncation.

### `temperature`

`temperature = 0.0` is greedy — sample the single highest-probability
token at every step. **It's also the fastest** because the sampler is
trivial.

`temperature > 0.0` enables the random sampler (with optional `seed`
for reproducibility), which is marginally slower per token. The
difference is small enough that you should pick based on output
quality, not speed.

## Hardware-side knobs

### Quantization

A `Q4_K_M` model is roughly 2× faster than `Q8_0` of the same model —
fewer bits to fetch from memory per matrix multiply. See
[Choosing a model](../guide/models.md#quantization) for the
size/quality table.

If `Q4_K_M` answers are good enough for your use case, prefer it
over `Q8_0`. The space and speed savings are real; the quality drop
is usually small for chat workloads.

### GPU offload

The biggest single speedup is moving compute off CPU. On Apple
Silicon, see [Apple Metal](./metal.md) — `n_gpu_layers: 999`
typically gives a 3–4× speedup for medium models.

On Linux + NVIDIA, CUDA support exists in `llama-cpp-2` but isn't
surfaced as an `ext-infer` cargo feature yet. Open an [issue](https://github.com/DisplaceTech/ext-infer/issues)
if you want it.

### Pinning threads to cores

llama.cpp respects the `OMP_NUM_THREADS` environment variable.
Setting it explicitly is sometimes faster than the default (which
uses all available cores, including hyperthreads that hurt more than
help). For a 4-physical-core box:

```sh
OMP_NUM_THREADS=4 php hello.php
```

Experimentally find the sweet spot for your CPU.

## Measuring before tuning

A useful pattern: log latency per call and look for the actual
bottleneck before reaching for any of these knobs.

```php
$start = hrtime(as_number: true);
$r = $model->chat($prompt, maxTokens: 512);
$elapsed_ms = (hrtime(true) - $start) / 1_000_000;

error_log(sprintf(
    'chat: %.0fms, %d tokens, %.1f tok/s, finish=%s',
    $elapsed_ms,
    $r->tokensGenerated(),
    $r->tokensGenerated() / ($elapsed_ms / 1000),
    $r->finishReason(),
));
```

If tokens/sec is low (< 20 on a modern CPU), you're hardware-bound —
quantize down or enable GPU offload. If it's reasonable (50+) but
total time is high, you're generating too many tokens — reduce
`maxTokens` or compress the prompt.

## Future work

Two performance items on the roadmap that aren't shipping in v0.1
but would change the picture significantly:

- **Reusable session contexts** — KV-cache reuse across `chat()`
  calls. Multi-turn conversations would skip the prefill cost on
  every turn after the first.
- **Continuous batching** — process N prompts together so the GPU
  stays saturated. Necessary for any serious inference-as-a-service
  workload.

Tracked in [`PLAN.md`](https://github.com/DisplaceTech/ext-infer/blob/main/PLAN.md).

## Next

- [Apple Metal](./metal.md) — usually the largest single
  improvement on macOS.
- [Choosing a model](../guide/models.md) — the model you pick caps
  everything else.
- [Worker pools recipe](../recipes/worker-pools.md) — when per-call
  tuning isn't enough.
