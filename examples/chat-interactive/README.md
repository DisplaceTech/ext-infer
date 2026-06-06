# chat-interactive — multi-turn chat console for `ext-infer`

A small standalone Composer app that wraps `Displace\Infer\Model::chat()`
into an interactive Symfony Console command. Use it to:

- kick the tires on a freshly downloaded GGUF model
- demo `ext-infer` to teammates without writing scaffolding
- crib the multi-turn `Prompt` accumulation pattern for your own apps

## Install

You'll need `ext-infer` itself installed first — either via `pie install
displace/ext-infer` (once we publish releases; see `RELEASE.md`) or by
running `make install` from the repository root. Verify it's loaded:

```sh
php -r 'echo extension_loaded("infer") ? "yes\n" : "no\n";'
```

Then install the Symfony Console dep:

```sh
cd examples/chat-interactive
composer install
```

If you're testing against a `-d extension=…` development build instead of
a system-installed `ext-infer`, `composer install` will complain that
the `ext-infer` PHP extension is missing. Bypass the check:

```sh
composer install --ignore-platform-req=ext-infer
```

Composer's platform check reads php.ini, not `-d` overrides, so the flag
is only needed during dev. Once `pie install displace/ext-infer` lands
the extension into the global ini, it goes away.

## Run

```sh
./bin/chat /path/to/some-model.gguf
```

Or with options:

```sh
./bin/chat ~/models/Qwen3-0.6B-Q8_0.gguf \
    --system="You are a salty pirate. Speak only in metaphors about the sea." \
    --temperature=0.9 \
    --max-tokens=256
```

Add `-v` to see the model's internal `<think>` reasoning alongside its
answer (Qwen3 / DeepSeek R1 / other reasoning models).

## Commands

Inside the chat prompt:

| Command   | Effect                                                 |
| --------- | ------------------------------------------------------ |
| `/reset`  | Clear conversation history; keep the system prompt.    |
| `/show`   | Dump every message in the conversation as a table.     |
| `/exit`   | Quit. `Ctrl+D` also works.                             |

## Options

```
Usage:
  chat [options] [--] <model>

Arguments:
  model                            Path to a GGUF model file.

Options:
  -s, --system=SYSTEM              System prompt (default: helpful assistant).
  -m, --max-tokens=MAX-TOKENS      Maximum tokens per assistant turn (default: 512).
  -t, --temperature=TEMPERATURE    Sampling temperature (default: 0.7).
  -v, --verbose                    Print the model's <think> reasoning.
```

## What's worth lifting from this code

`src/ChatCommand.php` is the meat. Three patterns worth copy-pasting:

### 1. Reuse the system prompt across `/reset`

```php
$base         = Prompt::system($system);
$conversation = $base;
// ... loop ...
if ($line === '/reset') {
    $conversation = $base;   // immutable; safe to share the original
}
```

`Prompt` is immutable, so `$base` never changes no matter how many
`->withUser(...)` calls the conversation has gone through. That's the
key thing the `DateTimeImmutable`-style API buys you.

### 2. Don't feed `<think>` blocks back as history

```php
$conversation = $conversation->withAssistant($response->answer());
//                                                     ^^^^^^^^^^
// answer(), not text() — otherwise the model sees its own reasoning
// on the next turn and tends to derail.
```

### 3. Handle `length` truncation visibly

```php
if ($response->finishReason() === 'length') {
    $io->writeln('<comment>(truncated — bump --max-tokens)</comment>');
}
```

Better to surface a hint than to leave the user wondering why the
answer cut off mid-word.

## Extending

The command lives in one class (`ChatCommand`) by design — drop it into a
Symfony app and it'll just work as a sub-command. To add a new command
(say, `embed`), add a `src/EmbedCommand.php`, register it in `bin/chat`,
and the Symfony `Application` will dispatch on argv.
