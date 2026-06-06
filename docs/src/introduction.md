# Introduction

`ext-infer` is a PHP 8.3+ extension that loads a [GGUF](https://github.com/ggerganov/ggml/blob/master/docs/gguf.md)
model and runs LLM inference inside the PHP process via
[llama.cpp](https://github.com/ggerganov/llama.cpp). PHP-native semantic
search, RAG pipelines, and CLI / worker inference run without shelling
out to Python or hitting a remote API.

It is written in [Rust](https://www.rust-lang.org/) on top of
[`ext-php-rs`](https://github.com/davidcole1340/ext-php-rs) and the
[`llama-cpp-2`](https://crates.io/crates/llama-cpp-2) bindings. The public
PHP surface is designed to feel native: a fluent, role-aware `Prompt`
builder; a `Response` that splits reasoning from answer; an `Embedding`
that knows how to normalize itself and compute cosine similarity. You
should rarely, if ever, need to think about `<|im_start|>` tokens.

## Why an extension?

Three reasons local inference belongs *in* PHP rather than next to it:

- **Latency.** A subprocess fork or HTTP roundtrip is at least
  milliseconds, often tens. An in-process call is bounded only by
  decode time.
- **Operational surface.** No Python sidecar to package, no daemon to
  supervise, no inference server to scale alongside FPM. The PHP
  process *is* the inference server.
- **API ergonomics.** Calling a local LLM should be as natural in PHP
  as calling `intl` or `pdo`. The extension API is shaped to match
  that — see [Prompts](./guide/prompts.md) and
  [Chat completions](./guide/chat.md).

## What's here

This guide is split into five layers, navigable from the sidebar:

| Section                          | What you'll find                                                                                              |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| [Getting Started](./getting-started/installation.md) | Install, run hello-world, verify it loaded.                                                       |
| [Guide](./guide/prompts.md)      | Conceptual walkthroughs of each public class. Read in order on first pass.                                    |
| [Recipes](./recipes/multi-turn-chat.md) | Copy-paste-ready patterns: multi-turn chat, semantic search, RAG, worker pools.                          |
| [Reference](./reference/api.md)  | Complete API listing, exceptions, environment variables, compatibility matrix.                                |
| [Advanced](./advanced/threading.md) | Threading model, Apple Metal, performance tuning.                                                              |

## Status

`ext-infer` is **pre-release** — the class surface is stable but the
first tagged release (`v0.1.0`) is still in flight. See
[`RELEASE.md`](https://github.com/DisplaceTech/ext-infer/blob/main/RELEASE.md)
for the cut-a-release flow and [`PLAN.md`](https://github.com/DisplaceTech/ext-infer/blob/main/PLAN.md)
for what's coming next.

## Conventions in this guide

- **Code blocks** are runnable as written, with one exception: PHP code
  assumes the extension is loaded. Either install it system-wide or
  prepend `-d extension=…` to your `php` command. See
  [Installation](./getting-started/installation.md).
- **`Model`** without a namespace prefix means `Displace\Infer\Model`;
  same for `Prompt`, `Response`, `Embedding`. Real code needs the `use`
  statement at the top of the file.
- **CLI snippets** are written for a POSIX shell (bash / zsh). Adjust
  for fish / PowerShell as needed; differences are usually only quoting.
