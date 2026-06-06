# Raw completions

`Model::raw()` is the escape hatch for callers who want full control
over the prompt string instead of going through
[`Prompt`](./prompts.md) + [`Model::chat()`](./chat.md).

```php
public function raw(
    string $prompt,
    int    $maxTokens   = 128,
    int    $nCtx        = 2048,
    float  $temperature = 0.0,
    int    $seed        = 1234,
    bool   $addBos      = true,
): string;
```

Returns a plain string — no `Response` wrapper, no reasoning split. If
you want either of those, use [`chat()`](./chat.md).

## When to use raw()

Three legitimate use cases:

### 1. Models without a chat template

"Base" / "pretrained" / "foundation" models — Llama 3 base, Mistral
base, Qwen base — ship GGUFs that haven't been instruction-tuned and
have no embedded chat template. `Model::chat()` rejects them with:

```
InferenceException: model has no embedded chat template — use
Model::raw() for this model
```

For these models, `raw()` is the *only* path. Build prompts in
whatever shape the model expects — typically just free-form
text-continuation:

```php
$text = $model->raw(
    "The capital of France is",
    maxTokens: 8,
    temperature: 0.0,
);
// " Paris."
```

### 2. Custom chat templates

Maybe the model's *embedded* chat template doesn't match what you want
— e.g. you want to add a tool-result message that the embedded template
doesn't know about, or you're injecting RAG context in a non-standard
slot. Build the prompt string yourself:

```php
$prompt = <<<TXT
<|im_start|>system
You are a calculator. Only emit JSON: {"result": <number>}.
<|im_end|>
<|im_start|>user
What is 2+2?
<|im_end|>
<|im_start|>assistant

TXT;

$text = $model->raw($prompt, maxTokens: 32, temperature: 0.0);
// '{"result": 4}'
```

The trade-off: you own template correctness. The chat template that
[`chat()`](./chat.md) uses is the one the model author tested with;
hand-rolling means hand-checking.

### 3. Stop-sequence simulation

Stop-string support is on the roadmap but not in v0.1. If you need a
generation to halt at a specific marker, `raw()` plus post-processing
is the workaround:

```php
$text = $model->raw($promptEndingWithMarker, maxTokens: 256);
$text = substr($text, 0, strpos($text, '</answer>') ?: strlen($text));
```

## What raw() does NOT do

- **No reasoning split.** `raw()` returns a string, not a `Response`.
  If the model emits `<think>…</think>` blocks, they end up in your
  string verbatim. You can strip them yourself with a regex if it
  matters; the canonical case for that is [Reasoning
  models](../recipes/reasoning-models.md).
- **No chat-template rendering.** What you pass in is what gets
  tokenized.
- **No finish-reason or token-count metadata.** If you need those,
  use `chat()`.

## addBos

The `addBos: true` default tells the tokenizer to prepend the model's
beginning-of-sequence token (whatever it is for that family). For most
models that's right. Set `addBos: false` when:

- You're building a prompt that already starts with the BOS token
  explicitly (rare).
- The model's tokenizer rejects BOS prepending (also rare).
- You're feeding `raw()` mid-conversation and don't want a new BOS in
  the middle (very rare and probably a sign you should be using a
  `Prompt`).

The other named arguments — `maxTokens`, `nCtx`, `temperature`, `seed`
— behave the same as in [`chat()`](./chat.md). See [Options
reference](./options.md).

## When NOT to use raw()

If the model has a chat template and you're sending the
"system / user / assistant" shape: use [`chat()`](./chat.md). It's
shorter, safer, and the `Response` it returns gives you reasoning
splitting + metadata for free.

`raw()` exists so escape hatches don't require dropping the extension
entirely. Treat it as the lower-level layer, not the default.

## Next

- [Chat completions](./chat.md) — the higher-level surface most code
  should use.
- [Options reference](./options.md) — every argument explained side
  by side.
