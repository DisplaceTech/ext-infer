# Compatibility matrix

## PHP versions

| Version | Status                                            | Notes                                                       |
| ------- | ------------------------------------------------- | ----------------------------------------------------------- |
| 8.3     | ✅ supported                                       | Security-only upstream through end of 2026.                 |
| 8.4     | ✅ supported                                       | Active support.                                              |
| 8.5     | ✅ supported                                       | Current release.                                             |
| 8.2 and earlier | ❌ not supported                                | `composer.json` declares `php: ^8.3`.                       |

Every released binary is built against a specific PHP minor. A binary
built for PHP 8.4 will not load into PHP 8.5 or 8.3. PIE handles this
automatically (it picks the right tarball); manual installs need to
match versions explicitly.

## Operating systems

| Platform               | Status                                            | Notes                                                       |
| ---------------------- | ------------------------------------------------- | ----------------------------------------------------------- |
| macOS arm64            | ✅ supported                                       | Apple Silicon. Tested on macOS 14+.                          |
| macOS x86_64           | ⚠️  not in release matrix                          | Builds from source. We don't ship binaries.                  |
| Linux x86_64 (glibc)   | ✅ supported                                       | Ubuntu 22.04+, Debian 12+, RHEL 9+. Most modern distros.     |
| Linux arm64 (glibc)    | ✅ supported                                       | Ubuntu 24.04 arm64, Debian 12 arm64, AWS Graviton.           |
| Linux musl (Alpine)    | ⚠️  builds from source                             | `.cargo/config.toml` has the right `crt-static` opt-out; no released binary. |
| FreeBSD / OpenBSD      | ⚠️  builds from source                             | Untested but should work; the build script handles non-Linux non-macOS as Linux. |
| Windows                | ❌ excluded                                        | `os-families-exclude: ["windows"]` in `composer.json`. Out of scope for v0.1. |

## Threading

`ext-infer` is **thread-safe by design** — the `LlamaBackend`
singleton is guarded by a `Sync` mutex, the underlying `LlamaModel`'s
weights are read-only after load (llama.cpp explicitly supports many
contexts on one model), and each `chat()` / `raw()` / `embed()` call
builds its own per-call `LlamaContext`. Two threads calling
`Model::chat()` concurrently on the same handle is the supported,
intended shape.

| PHP build  | Status                                            | Notes                                                       |
| ---------- | ------------------------------------------------- | ----------------------------------------------------------- |
| NTS        | ✅ supported, the default                          | What every release binary targets today.                     |
| ZTS        | ✅ supported (`support-zts: true` in `composer.json`) | Not yet exercised in CI. See [Threading & ZTS](../advanced/threading.md). |

## Acceleration backends

| Backend           | Status                                            | Notes                                                       |
| ----------------- | ------------------------------------------------- | ----------------------------------------------------------- |
| CPU (default)     | ✅ supported, the default                          | Portable, no hardware requirements.                          |
| Apple Metal       | ⚠️ opt-in via cargo feature                        | `make release FEATURES=metal`. See [Apple Metal](../advanced/metal.md). |
| CUDA (NVIDIA GPU) | ❌ not yet                                         | llama-cpp-2 supports it via a cargo feature; we haven't exposed or tested it. |
| ROCm / Vulkan     | ❌ not yet                                         | Same — supported upstream, not surfaced.                     |

If you want CUDA or other GPU acceleration sooner rather than later,
open an issue describing your use case — surfacing the feature is
small work; *testing* it across the GPU landscape is the hard part.

## Tested model families

What the maintainers have actually exercised end-to-end. Other
GGUF-supported families almost certainly work; this is the "we've
seen it produce sensible output" list.

| Family                | Used for                                          |
| --------------------- | ------------------------------------------------- |
| Qwen3 (Instruct)      | Chat completions, reasoning splitting.            |
| Qwen3-Embedding       | Embeddings, cosine similarity.                    |
| Llama 3 / 3.1 / 3.2   | Chat completions. No reasoning.                   |
| Mistral               | Chat completions.                                 |
| BGE / E5 / GTE        | Embeddings.                                       |

## Versioning policy

- Pre-1.0 (`0.x.y`), breaking changes happen between minors (`0.1.x` →
  `0.2.x`), not patches.
- Once `v1.0.0` ships, the class / method / argument surface is
  frozen. New features land additively; behavioral changes that affect
  existing callers wait for the next major.
- See [`RELEASE.md`](https://github.com/DisplaceTech/ext-infer/blob/main/RELEASE.md)
  for the cut-a-release flow.

## Reporting compatibility issues

If you hit a "should work but doesn't" combination on this matrix, the
[issue template](https://github.com/DisplaceTech/ext-infer/issues/new)
asks for:

- PHP version (`php --version`)
- OS / arch (`uname -a`)
- libc (Linux: `ldd --version | head -1`)
- ZTS or NTS (`php -i | grep 'Thread Safety'`)
- Whether the extension was installed via PIE, `make install`, or
  loaded with `-d extension=…`

Three of those four are usually enough to triage.
