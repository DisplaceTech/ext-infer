# Apple Metal

[Metal](https://developer.apple.com/metal/) is Apple's low-level GPU
API. On Apple Silicon hardware (M1 / M2 / M3 / M4), llama.cpp uses
Metal to offload weight-matrix multiplications to the integrated GPU,
which substantially outpaces the CPU for medium-to-large models.

`ext-infer` exposes Metal as an opt-in cargo feature. It is **not**
enabled by default — the default build is CPU-only and portable to
non-Apple platforms.

## When Metal helps

Order-of-magnitude rule of thumb on an M-series Mac:

| Model size | CPU tokens/sec | Metal tokens/sec | Speedup |
| ---------- | -------------- | ---------------- | ------- |
| 0.6B       | ~80            | ~120             | 1.5×    |
| 3B         | ~25            | ~70              | 2.8×    |
| 7B         | ~12            | ~50              | 4×      |
| 13B+       | (memory-limited) | ~25            | dramatic |

Numbers are rough — they depend on quant level, M-series generation,
prompt length, and what else the machine is doing. The pattern is
clear though: Metal's value grows with model size.

For small models on a fast CPU, Metal can actually be *slower* on the
first few tokens because of the shader compilation overhead. If
you're running 600M-param models in batch mode, the CPU build is
likely fine.

## Enabling Metal

The cargo feature is named `metal`:

```sh
make release FEATURES=metal
make install  FEATURES=metal
```

Or via raw cargo:

```sh
cargo build --release --features metal
```

The release binary is now Metal-enabled. No runtime flag — Metal is
used automatically when the cargo feature is on.

### Per-layer offload

The `Model::load()` option `n_gpu_layers` controls how many
transformer layers are offloaded to the GPU. Defaults to `0` (CPU
only); set to a high number (the model's total layer count, or just
`999` as a "all of them" shortcut) to offload everything:

```php
$model = Model::load($path, [
    'n_gpu_layers' => 999,   // offload all layers to Metal
]);
```

For models that fit entirely in unified memory, full offload is
almost always what you want. For models that *don't* fit, partial
offload lets you put the hot lower layers on the GPU and keep the
upper layers on CPU. Tune empirically; the upstream
[llama.cpp Metal docs](https://github.com/ggerganov/llama.cpp/blob/master/docs/backend/Metal.md)
have more.

## Why isn't it the default?

Three reasons we ship CPU-by-default and Metal-by-opt-in for v0.1:

1. **The release matrix builds on the GitHub `macos-14` runner.** Its
   hardware revision and `MACOSX_DEPLOYMENT_TARGET` are
   not-fully-pinned — we haven't validated that a Metal-enabled binary
   built there actually loads on every customer Mac.
2. **CI doesn't test Metal output for correctness.** Different
   precision behavior on GPU vs CPU could surface as different
   sampler output, and we haven't caught that drift end-to-end yet.
3. **Cold-start cost.** Metal shader compilation adds ~1s to the
   first inference. Acceptable for long-running workers, awkward for
   a CLI tool people run once.

Making Metal the default for macos-arm64 release tarballs is on the
[roadmap](https://github.com/DisplaceTech/ext-infer/blob/main/PLAN.md)
once those three concerns are resolved.

## Verifying Metal is actually being used

Enable [`EXT_INFER_LOG`](../reference/environment.md) and look for
Metal-specific lines:

```sh
EXT_INFER_LOG=1 php hello.php 2>&1 | grep -i metal | head
```

You should see something like:

```
ggml_metal_init: GPU name:   Apple M2 Max
ggml_metal_init: GPU family: MTLGPUFamilyApple8 (1008)
ggml_metal_init: hasUnifiedMemory              = true
ggml_metal_init: recommendedMaxWorkingSetSize  = 48318.38 MiB
```

If you see no Metal lines at all, the cargo feature didn't get
applied — re-check the `make release FEATURES=metal` invocation.

## Memory considerations

Apple Silicon has *unified memory* — the GPU and CPU share the same
physical RAM. There is no "host-to-device" copy step like on
discrete GPUs. The trade-off is that GPU memory pressure shows up as
overall system memory pressure: a 13B model in Metal mode uses ~8 GB
of the same RAM your other apps need.

`recommendedMaxWorkingSetSize` in the log above is what macOS thinks
you should keep the GPU footprint under. Loading a model larger than
that *will* work — Metal pages weights in and out as needed — but
performance drops sharply.

## Cross-platform note

`#[cfg(feature = "metal")]` only enables Metal on Apple targets.
Building with `--features metal` on Linux is harmless (the feature is
a no-op there), but there's no reason to do it.

For GPU acceleration on non-Apple hardware (CUDA on NVIDIA, ROCm on
AMD, Vulkan as a portable option) — the `llama-cpp-2` crate supports
all three, but `ext-infer` hasn't surfaced them as cargo features yet.
If you want one, [open an issue](https://github.com/DisplaceTech/ext-infer/issues).

## Next

- [Performance tuning](./performance.md) — once Metal is on, the
  next bottleneck is usually `nCtx` or `maxTokens`.
- [Choosing a model](../guide/models.md) — Metal opens up larger
  models you might not have considered.
