# Multi-turn chat

The pattern: keep the system message stable, append user/assistant
turns as the conversation grows, regenerate the prompt on each user
input. Lifts directly from [`examples/chat-interactive/`](https://github.com/DisplaceTech/ext-infer/tree/main/examples/chat-interactive).

## The shape

```php
use Displace\Infer\Model;
use Displace\Infer\Prompt;

$model = Model::load('models/Qwen3-0.6B-Q8_0.gguf');

$base         = Prompt::system('You are a helpful, concise assistant.');
$conversation = $base;

while (($line = readline('> ')) !== false) {
    $line = trim($line);
    if ($line === '' || $line === '/exit') {
        break;
    }

    // /reset is trivial because Prompt is immutable.
    if ($line === '/reset') {
        $conversation = $base;
        continue;
    }

    $conversation = $conversation->withUser($line);

    $response = $model->chat(
        $conversation,
        maxTokens: 512,
        temperature: 0.7,
    );

    echo $response->answer(), PHP_EOL;

    // Feed answer() back, NOT text(). See "Reasoning models" below.
    $conversation = $conversation->withAssistant($response->answer());
}

$model->close();
```

## Three things this gets right

### 1. The system message is stable

`$base` is built once and never mutated. Every `/reset` re-seats the
conversation at the original system instruction without re-allocating
or re-rendering. If you change the system prompt mid-conversation
elsewhere in your app, the immutable shape means concurrent uses of
`$base` aren't affected.

### 2. Conversation grows by immutable append

```php
$conversation = $conversation->withUser($line);
```

Every `with*` returns a new `Prompt`. The old `$conversation` is
still valid (and still has the previous turn count); the local just
points at the new one. There's no shared mutable state, so this code
is safe to put behind a queue worker or run in parallel.

### 3. `Response::answer()` goes back, not `text()`

```php
$conversation = $conversation->withAssistant($response->answer());
```

This matters for reasoning models. `answer()` is the reply with
`<think>...</think>` blocks stripped; `text()` is the raw output
including the thoughts. Feeding `text()` back means the model sees its
own internal monologue on the next turn — and reasoning models tend to
treat that as instruction, not history. The output derails fast.

For non-reasoning models, `answer()` and `text()` are byte-identical,
so the rule is "use `answer()` always" rather than "use `answer()` for
some models".

## Persisting conversations

If you need to save and restore conversations (e.g. per-user chat
history in a database), serialize the message list and rebuild the
`Prompt`:

```php
function loadConversation(string $system, array $history): Prompt
{
    $p = Prompt::system($system);
    foreach ($history as $row) {
        $p = match ($row['role']) {
            'user'      => $p->withUser($row['content']),
            'assistant' => $p->withAssistant($row['content']),
        };
    }
    return $p;
}

function saveConversation(Prompt $p): array
{
    $rows = [];
    foreach ($p->messages() as $msg) {
        $rows[] = ['role' => $msg->role(), 'content' => $msg->content()];
    }
    return $rows;
}
```

`Prompt::messages()` walks in chronological order, so saving and
re-loading round-trips faithfully.

## Common shape: an HTTP turn

For a request/response API where every HTTP call is one turn:

```php
// Inside your controller — assumes $model is injected and reused.
final class ChatController
{
    public function __construct(private Model $model, private HistoryStore $history) {}

    public function turn(Request $req): Response
    {
        $conversationId = $req->session('conversation_id');
        $history        = $this->history->load($conversationId);
        $system         = $req->user()->systemPrompt() ?? 'You are helpful.';

        $prompt = loadConversation($system, $history)
            ->withUser($req->json('message'));

        $reply = $this->model->chat(
            $prompt,
            maxTokens: 1024,
            temperature: 0.5,
        );

        $this->history->append($conversationId, 'user', $req->json('message'));
        $this->history->append($conversationId, 'assistant', $reply->answer());

        return new JsonResponse([
            'answer'    => $reply->answer(),
            'reasoning' => $reply->reasoning(),
            'truncated' => $reply->finishReason() === 'length',
            'tokens'    => $reply->tokensGenerated(),
        ]);
    }
}
```

The `$model` is loaded once at FPM-worker boot — not per request — and
`chat()` is called per request. With current `ext-infer` (no
KV-cache reuse yet), each turn re-tokenizes and re-decodes the full
history, which is slow for long conversations. A `Session` object that
reuses the underlying llama.cpp context is on the [roadmap](https://github.com/DisplaceTech/ext-infer/blob/main/PLAN.md).

## When to use `Model::raw()` instead

If you have a very specific prompt shape — tool calls, RAG context
injected at a non-standard slot, custom format — see
[Raw completions](../guide/raw.md). The `Prompt` builder doesn't
support tool-call messages today, so tool-aware conversations need
`raw()` until [tool calling](../advanced/threading.md#future-work)
lands.
