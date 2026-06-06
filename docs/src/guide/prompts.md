# Prompts

`Displace\Infer\Prompt` is the input to [`Model::chat()`](./chat.md). It
represents an ordered list of role-tagged messages — system, user,
assistant — that the extension renders into whatever chat-template
format the underlying model expects. You never write `<|im_start|>` (or
its Llama 3 / Mistral / Gemma equivalent) by hand.

## Two-stage construction

A `Prompt` starts with a *factory* — either `system()` or `user()` —
and grows via `with*` calls. Each `with*` returns a **new** `Prompt`;
the receiver is never modified.

```php
use Displace\Infer\Prompt;

// Start with a system message:
$p = Prompt::system('You are a helpful assistant.')
    ->withUser('What is 2+2?');

// Or start with a user message (no system instruction):
$p = Prompt::user('Hello!');

// Multi-turn replays:
$p = Prompt::system('You are a poet.')
    ->withUser('Write a haiku about Rust.')
    ->withAssistant("Code runs cold and fast,\nMemory safe by the borrow,\nNo crashes today.")
    ->withUser('Now translate it to French.');
```

Direct `new Prompt()` is refused at runtime:

```php
new Prompt();
// Displace\Infer\InferException: use Displace\Infer\Prompt::system()
// or Prompt::user() to start a prompt
```

## Why immutable?

The shape mirrors `DateTimeImmutable`. Two practical consequences:

- A `Prompt` you've built once is safe to share across multiple
  `chat()` calls, hand to a queue worker, or stash in a class
  property. Nothing downstream can mutate it.
- Branching is free. The
  [multi-turn chat recipe](../recipes/multi-turn-chat.md) keeps a
  `$base` `Prompt` around (system-message-only) so `/reset` can drop
  conversation history without re-rendering the system prompt:

  ```php
  $base         = Prompt::system($systemMessage);
  $conversation = $base;
  // … many turns …
  if ($userTyped === '/reset') {
      $conversation = $base;   // immutable; $base is untouched no
                               // matter how many turns went through it
  }
  ```

## Inspecting a Prompt

```php
$p->messages();    // list<Displace\Infer\Message>
$p->count();       // int — number of messages
$p->isEmpty();     // bool
$p->lastRole();    // ?string — role of the most recent message, or null
```

Each `Message` is read-only:

```php
foreach ($p->messages() as $msg) {
    printf("[%s] %s\n", $msg->role(), $msg->content());
}
// [system] You are a helpful assistant.
// [user] What is 2+2?
```

`role()` is always one of `'system'`, `'user'`, or `'assistant'`.
Method-name discipline on the construction side (`withSystem`,
`withUser`, `withAssistant`) keeps typos from creating fictional roles
at compile time.

## Role ordering

`ext-infer` does not enforce role ordering at construction time. You
can build:

```php
Prompt::user('hi')->withSystem('be terse');  // legal
Prompt::system('a')->withSystem('b');         // also legal
```

…and they will be rendered as written. Whether the model accepts the
result is a chat-template decision: most modern chat templates require
exactly one leading `system` message (or none) followed by alternating
`user` / `assistant` turns. Build sequences that match that convention
and the chat template will render them; deviate and you may get an
error from
[`Model::chat()`](./chat.md#chat-template-errors) at call time.

## Composition patterns

### Pre-baked system prompts

If your application has a few stock personalities, define them once:

```php
final class Personas
{
    public static function poet(): Prompt
    {
        return Prompt::system(
            'You are a haiku poet. Respond in three lines. ' .
            'Five syllables, then seven, then five.'
        );
    }

    public static function reviewer(): Prompt
    {
        return Prompt::system(
            'You review code. Always cite specific line numbers ' .
            'and prefer questions over assertions when uncertain.'
        );
    }
}

$response = $model->chat(Personas::poet()->withUser('Tell me about autumn.'));
```

Because `Prompt` is immutable, returning a `Prompt` from a helper
method is safe — callers can't mutate the cached base.

### Replaying history

When you have stored history (e.g. fetched from a database), rebuild
the `Prompt` from scratch each turn:

```php
$prompt = Prompt::system($systemMessage);
foreach ($historyFromDb as $row) {
    $prompt = match ($row['role']) {
        'user'      => $prompt->withUser($row['content']),
        'assistant' => $prompt->withAssistant($row['content']),
    };
}
$prompt = $prompt->withUser($newUserInput);
```

This is the canonical multi-turn-chat shape. See the
[multi-turn chat recipe](../recipes/multi-turn-chat.md).

### Feeding `Response::answer()` back, not `text()`

When you append the assistant's reply to the prompt for the next turn,
use `Response::answer()` (reasoning stripped), **not**
`Response::text()`:

```php
$response = $model->chat($prompt);
$prompt   = $prompt->withAssistant($response->answer());
//                                          ^^^^^^^^^
//                            not ->text(), which includes <think>…</think>
```

Feeding `<think>` blocks back as conversation history derails reasoning
models — they see their own thoughts in the transcript and get
confused. See [Reasoning models](../recipes/reasoning-models.md).

## Next

- [Chat completions](./chat.md) — feeding a `Prompt` to the model.
- [`Model::raw()`](./raw.md) — when you want full control over the
  prompt string instead.
