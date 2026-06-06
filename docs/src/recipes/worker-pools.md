# Worker pools

LLM inference is slow: tens of milliseconds at best, often seconds.
Running it inline in an FPM worker means that worker is unavailable
for any other request until the model is done. For any non-trivial
deployment, you want a pool of workers — process-based, thread-based,
or queue-based — that absorbs the latency without starving the rest of
your app.

`ext-infer` is designed to slot into all three patterns.

## Pattern 1 — FPM workers (process-based)

The simplest production setup: PHP-FPM with `pm.max_children` set
high enough to absorb concurrent slow inference requests.

```ini
; php-fpm.d/www.conf
pm = dynamic
pm.max_children = 16
pm.start_servers = 4
pm.min_spare_servers = 2
pm.max_spare_servers = 8
pm.process_idle_timeout = 60s
```

Each FPM worker is its own OS process. They each load their own
`Model` once at warm-up and reuse it for the lifetime of the worker.
The model weights are mmap'd, so the OS shares physical memory across
workers — 16 workers loading the same 4 GB model use ~4 GB of RAM
total, not 64.

```php
// Shared service container — boot once per worker.
$model = Displace\Infer\Model::load($cfg['model_path']);

// In your request handler:
$response = $model->chat($prompt, maxTokens: 512);
```

The downside: each worker can handle one inference at a time. If you
hit `pm.max_children` concurrent requests, the (max_children + 1)st
request waits. Bump `max_children` if you have the RAM (the model is
shared via mmap; only the KV cache scales with concurrency); push
inference to a queue if you don't.

### Sizing

A rough sizing heuristic for FPM with `ext-infer`:

```
max_children ≈ (RAM_budget - model_size) / per_request_memory
```

Where `per_request_memory` is the KV cache footprint plus PHP's
working set — usually 100–500 MB per worker depending on `nCtx`.

## Pattern 2 — Job queue (process-based, decoupled)

For inference that takes long enough that you don't want it in the
request path at all:

```php
// In the request handler — enqueue, return immediately.
$jobId = $queue->push(InferJob::class, [
    'prompt'  => $prompt,
    'options' => ['maxTokens' => 512],
]);
return new JsonResponse(['job_id' => $jobId, 'status' => 'queued']);

// Client polls /jobs/{id} until status = 'done'.
```

```php
// In your queue worker — long-lived, model loaded once at boot.
final class InferWorker
{
    public function __construct(private \Displace\Infer\Model $model) {}

    public function process(InferJob $job): InferResult
    {
        $r = $this->model->chat($job->prompt, ...$job->options);
        return new InferResult($r->answer(), $r->finishReason());
    }
}
```

Any queue runner works — Symfony Messenger, Laravel Horizon,
ReactPHP's `react/event-loop`, a bespoke
`pcntl_fork` + `proc_open` script. The pattern is the same: one
`Model::load()` per worker process, reuse across many jobs.

This pattern shines when:

- Inference latency is unpredictable and you don't want to hold HTTP
  connections open.
- You want to scale inference workers independently of web workers.
- You want to route inference traffic across heterogeneous workers
  (CPU-only on cheap nodes, GPU-equipped on others).

## Pattern 3 — ZTS + `parallel` (thread-based)

For latency-sensitive workloads where the IPC overhead of pattern 2 is
too much, `ext-infer` supports concurrent calls *within a single
process* under ZTS PHP with the [`parallel`](https://www.php.net/manual/en/book.parallel.php)
extension.

This works because `ext-infer` is thread-safe by design:

- `LlamaBackend` is a `Sync`-guarded process-global singleton.
- `LlamaModel` (the weights) is immutable after load; llama.cpp
  explicitly supports many contexts on one model.
- Each `chat()` / `raw()` / `embed()` call builds its own per-call
  `LlamaContext` and drops it after.

Two threads calling `Model::chat()` simultaneously on the same handle
is the supported, intended shape.

```php
use parallel\Runtime;

// Load the model once in the main thread.
$model = Displace\Infer\Model::load('models/qwen3.gguf');

// Spin up a pool of runtimes.
$runtimes = array_map(fn() => new Runtime(), range(1, 4));

// Dispatch concurrent inferences.
$futures = [];
foreach ($prompts as $i => $prompt) {
    $rt = $runtimes[$i % 4];
    $futures[$i] = $rt->run(function (Model $m, Prompt $p) {
        return $m->chat($p, maxTokens: 512)->answer();
    }, [$model, $prompt]);
}

// Collect.
$answers = array_map(fn($f) => $f->value(), $futures);

$model->close();
```

### Caveats

- **ZTS PHP is uncommon.** Most distros ship NTS by default; you'll
  have to build ZTS PHP from source (`./configure --enable-zts`)
  or use a ZTS-shipping Docker image. PIE's pre-built binaries
  target NTS for v0.1; ZTS binaries are on the roadmap.
- **`parallel` itself requires ZTS.** Can't use it on a standard
  NTS install.
- **CI doesn't exercise this yet.** ZTS support is enabled in
  `composer.json` because the code is thread-safe by construction —
  but the maintainers have not yet run multi-threaded stress tests in
  CI. Treat it as "should work, please report bugs" until that
  changes. See [Threading & ZTS](../advanced/threading.md) for the
  current state.

## Choosing between the patterns

| Concern                                | FPM workers | Job queue   | ZTS + parallel |
| -------------------------------------- | ----------- | ----------- | -------------- |
| Easy to set up                         | ✅ trivial   | ⚠️  some IPC | ⚠️  ZTS build  |
| Holds HTTP connection during inference | yes         | no          | yes            |
| Survives PHP being NTS                 | ✅          | ✅          | ❌              |
| Shares one model across all concurrency| via mmap    | per-worker  | within process |
| Scales to many concurrent inferences   | ⚠️ workers eat RAM | ✅ horizontal | ⚠️ one process |
| Production-tested in `ext-infer`       | ✅          | ✅          | ⚠️ unexercised |

For most teams, **FPM with a generous `max_children`** is the right
starting point. Move to a queue when latency variance gets too high
for the request path. Reach for `parallel` last, when you've measured
that IPC overhead is the bottleneck.

## Next

- [Threading & ZTS](../advanced/threading.md) — what makes the
  `parallel` story actually work.
- [Performance tuning](../advanced/performance.md) — what knobs to
  pull when each worker is too slow.
