# Chat completions

`Model::chat()` is the main inference entry point. It takes a
[`Prompt`](./prompts.md) and returns a `Response`:

```php
public function chat(
    \Displace\Infer\Prompt $prompt,
    int   $maxTokens   = 128,
    int   $nCtx        = 2048,
    float $temperature = 0.0,
    int   $seed        = 1234,
): \Displace\Infer\Response;
```

All four sampling arguments are PHP 8 named arguments — no
options array. See [Options reference](./options.md) for what each
one does.

## What chat() does

Three steps happen between the call and the return value:

1. **Render.** The `Prompt`'s messages are fed through
   [`llama_chat_apply_template`](https://github.com/ggerganov/llama.cpp/blob/master/include/llama.h),
   using the chat template *embedded in the GGUF*. Qwen3, Llama 3,
   Mistral, Gemma — each ships its own Jinja template inside the model
   file. `ext-infer` reads it and uses it verbatim.

2. **Decode.** The rendered prompt is tokenized, decoded through the
   model in a single batch, then a sampler generates output tokens one
   by one until either the model emits an end-of-generation token
   (`finishReason = 'eos'`) or the `maxTokens` budget is exhausted
   (`finishReason = 'length'`).

3. **Split.** If the generated text contains `<think>...</think>`
   blocks (Qwen3 / DeepSeek R1 / other reasoning models), they're
   captured into `Response::reasoning()` and stripped from
   `Response::answer()`. See
   [Reasoning models](../recipes/reasoning-models.md) for the details.

## Inspecting a Response

`Response` is read-only. Six getters:

```php
$response->text();              // string — full output, <think>…</think> + answer
$response->reasoning();         // ?string — captured <think>…</think>, or null
$response->answer();            // string — text() minus reasoning, leading WS trimmed
$response->hasReasoning();      // bool
$response->finishReason();      // string — 'eos' | 'length' | 'stop'
$response->tokensGenerated();   // int — generated tokens only, not prompt
```

`Response` is created internally — `new Response()` throws.

### `text()` vs `answer()`

For non-reasoning models, the two are byte-identical. For reasoning
models invoked through their chat template:

```text
text():     <think>Okay so 2+2…</think>\n\n2 + 2 equals 4.
answer():   2 + 2 equals 4.
reasoning(): Okay so 2+2…
```

`answer()` is what end users want to read; `reasoning()` is what you'd
log for debugging or display behind a "show thinking" toggle.

### `finishReason()`

Three possible values:

| Value      | Meaning                                                              |
| ---------- | -------------------------------------------------------------------- |
| `'eos'`    | Model emitted an end-of-generation token. Output is complete.        |
| `'length'` | `maxTokens` was hit before EOS. Output is likely truncated mid-thought. |
| `'stop'`   | Reserved for future stop-string support. Currently only reachable when the prompt produced zero tokens (a degenerate input). |

When you see `'length'`, surface it to the user — "hit the token budget,
bump `maxTokens` to see more". Silently truncating is a bad UX.

### `tokensGenerated()`

Counts generated tokens only, not the prompt's tokens. Useful for
billing-like accounting, latency analysis, or capping conversation
length.

## Calling chat()

A minimal call uses every default:

```php
$response = $model->chat(Prompt::user('Hello!'));
```

A fully-specified one:

```php
$response = $model->chat(
    Prompt::system('You are a helpful, concise assistant.')
        ->withUser('What is the capital of Antarctica?'),
    maxTokens: 256,
    nCtx: 4096,
    temperature: 0.7,
    seed: 42,
);
```

Sampling defaults — `temperature: 0.0`, `seed: 1234` — give greedy,
deterministic output: the same prompt always produces the same reply.
Crank `temperature` up for varied / creative output; the `seed` only
matters when `temperature > 0`.

## Errors

`Model::chat()` raises [`InferenceException`](../reference/exceptions.md)
for any failure between "the model exists" and "we got tokens back".
The most common message strings:

| Substring                                  | Meaning                                                                                                 |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------- |
| `model has been closed`                    | You called `chat()` after `$model->close()`. Reload the model.                                          |
| `model has no embedded chat template`      | The GGUF is a base model, not an instruct/chat variant. Either pick a chat-tuned model or use [`Model::raw()`](./raw.md). |
| `apply_chat_template failed`               | The chat template rendered but llama.cpp rejected the result. Usually means the message-role sequence is one the template doesn't support (e.g. multiple system messages). |
| `prompt is N tokens but n_ctx is only M`   | The rendered prompt is longer than `nCtx`. Bump `nCtx` or shorten the prompt. |
| `chat message contains a null byte`        | A `Prompt`'s content has an embedded `\0`. Strip it before constructing the prompt. |

### Chat-template errors

If you load a model that doesn't ship a chat template — typically a
"base" or "pretrained" model rather than an instruct variant — you'll
see:

```
InferenceException: model has no embedded chat template — use
Model::raw() for this model: …
```

`Model::raw()` lets you do your own templating. See
[Raw completions](./raw.md).

## Streaming

`chat()` is currently synchronous: it returns the complete `Response`
once decoding finishes. A streaming variant — likely
`Model::chatStream(): \Generator` — is in the [roadmap](https://github.com/DisplaceTech/ext-infer/blob/main/PLAN.md).

For long generations under a request/response model where blocking is
unacceptable, the workable shortcut today is to set a tight `maxTokens`
and call `chat()` repeatedly with the previous turn appended to the
`Prompt`. That sacrifices KV-cache reuse but works.

## Next

- [Raw completions](./raw.md) — the escape hatch for templates the
  model didn't bake in.
- [Choosing a model](./models.md) — chat-tuned vs base, quantization,
  size.
- [Multi-turn chat recipe](../recipes/multi-turn-chat.md) — the
  immutable-Prompt accumulation pattern.
- [Reasoning models recipe](../recipes/reasoning-models.md) — making
  `reasoning()` / `answer()` work for you.
