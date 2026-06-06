# Reasoning models

Qwen3, DeepSeek R1, and other reasoning-tuned models think out loud
before answering. When invoked through their chat template, they emit
`<think>…</think>` blocks containing the internal monologue, then the
actual reply. `ext-infer` understands this convention and exposes the
two streams separately on [`Response`](../guide/chat.md).

## The split, in three calls

```php
use Displace\Infer\Model;
use Displace\Infer\Prompt;

$model = Model::load('models/Qwen3-0.6B-Q8_0.gguf');

$response = $model->chat(
    Prompt::user('What is 2+2?'),
    maxTokens: 512,
);

echo $response->text(), PHP_EOL;
// <think>
// Okay, the user is asking what 2+2 is. This is basic arithmetic.
// I should respond with the correct sum, which is 4. Let me also
// verify there's no trick here — adding two and two definitely
// equals four.
// </think>
//
// 2 + 2 equals 4.

echo $response->answer(), PHP_EOL;
// 2 + 2 equals 4.

echo $response->reasoning(), PHP_EOL;
// Okay, the user is asking what 2+2 is. This is basic arithmetic.
// I should respond with the correct sum, which is 4. ...

echo $response->hasReasoning() ? 'yes' : 'no', PHP_EOL;
// yes

$model->close();
```

For a *non*-reasoning model:

- `reasoning()` returns `null`
- `answer()` equals `text()` byte-for-byte
- `hasReasoning()` returns `false`

The split is opt-out: there's no flag to disable it. If the input
doesn't contain `<think>…</think>` tags, nothing changes.

## When the budget runs out mid-thought

Reasoning chains can be long. If `maxTokens` exhausts inside a
`<think>` block — before the closing `</think>` — the split fails
gracefully:

- `text()` contains the partial reasoning verbatim, with the open
  `<think>` tag and no closing tag.
- `reasoning()` returns any *previous* completed reasoning blocks, or
  `null` if none.
- `answer()` is the input with completed blocks removed and the
  partial thought left in place. **The partial thought is intentionally
  left in `answer()`** — silently swallowing it would hide the budget
  problem.
- `finishReason()` returns `'length'`.

The fix is always "bump `maxTokens`". A useful pattern is to surface
the truncation explicitly:

```php
$response = $model->chat($prompt, maxTokens: 256);

if ($response->finishReason() === 'length') {
    error_log(sprintf(
        'truncated: model wanted more than 256 tokens for "%s..."',
        substr($prompt->messages()[0]->content(), 0, 40),
    ));
}
```

The interactive chat example uses a softer
[hint](https://github.com/DisplaceTech/ext-infer/blob/main/examples/chat-interactive/src/ChatCommand.php):
"(truncated — bump --max-tokens to see more)".

## When you DON'T want reasoning at all

Two strategies, depending on what "don't want" means.

### Strategy A — hide it in the UI, keep it under the hood

Default everywhere. Show `$response->answer()` to the end user. Log
`$response->reasoning()` for debugging or display behind a "show
thinking" toggle. No model-level change.

### Strategy B — tell the model to skip thinking

Qwen3 has a `/no_think` directive that, when included as a system-message
suffix, suppresses the `<think>...</think>` block entirely. The model
still emits an empty `<think></think>` block (which the split handles —
`reasoning()` ends up being an empty string), but skips the actual
monologue:

```php
$prompt = Prompt::system('You are helpful. /no_think')
    ->withUser('What is 2+2?');

$response = $model->chat($prompt);

$response->hasReasoning();   // true (empty block)
$response->reasoning();      // "" (empty string)
$response->answer();         // "2 + 2 equals 4."
```

This is Qwen3-specific. DeepSeek R1 has a similar concept (`/no-cot`
in some prompts). Other reasoning models vary. Check the model card.

## Feeding history back

When building multi-turn conversations against a reasoning model, feed
`Response::answer()` back as the assistant's reply, **not**
`Response::text()`:

```php
$conversation = $conversation->withAssistant($response->answer());
//                                          ^^^^^^^^^^^^^^^^^^^
//                                          not ->text()
```

`text()` includes the `<think>…</think>` block. Adding it to the
conversation means the model sees its own reasoning on the next turn
and tends to treat it as instruction rather than history — output
quality drops fast.

This is the single most-common mistake when wiring up reasoning models
in `ext-infer`. See [Multi-turn chat](./multi-turn-chat.md) for the
full pattern.

## Performance note

Reasoning models spend many tokens on their internal monologue. A
typical Qwen3-0.6B answer to "what is 2+2?" generates ~150 tokens of
thinking before the 5-token answer. That's an order of magnitude more
work than a non-reasoning model would do for the same question.

If latency matters more than the highest-quality answer:

- Use `/no_think` (Strategy B above) to skip the monologue.
- Pick a non-reasoning model — Llama 3.x Instruct, Mistral Instruct,
  Qwen 2.5 Instruct (not Qwen3) all chat without thinking out loud.

See [Performance tuning](../advanced/performance.md) for more knobs.
