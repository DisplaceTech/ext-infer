# Structured output

Free-text generation is the wrong tool the moment your code needs to
*parse* what the model says. Grammar-constrained generation flips the
guarantee: instead of asking nicely for JSON and validating after the
fact, the sampler is constrained so that **every token the model can
emit keeps the output inside your schema**. Malformed output isn't
retried — it's impossible.

This is the reliability unlock for small local models: a 0.6B model
that rambles in free text becomes a dependable structured extractor
when it physically cannot produce anything but the requested shape.

## JSON Schema (the common case)

Pass a JSON Schema via the `schema` option — as a PHP array or a JSON
string — to `chat()` or `raw()`:

```php
use Displace\Infer\Model;
use Displace\Infer\Prompt;

$model = Model::load('models/Qwen3-0.6B-Q8_0.gguf');

$response = $model->chat(
    Prompt::system('Extract the data. Output JSON only.')
        ->withUser('Maria is 31 years old and lives in Lisbon.'),
    maxTokens: 128,
    options: ['schema' => [
        'type' => 'object',
        'properties' => [
            'name' => ['type' => 'string'],
            'age'  => ['type' => 'integer'],
            'city' => ['type' => 'string'],
        ],
    ]],
);

$data = json_decode($response->answer(), true, flags: JSON_THROW_ON_ERROR);
// ['name' => 'Maria', 'age' => 31, 'city' => 'Lisbon'] — guaranteed shape
```

`json_decode` cannot fail here: the grammar permits only valid JSON
matching the schema, with properties in declaration order.

Classification is one `enum` away:

```php
$sentiment = json_decode($model->raw(
    "Sentiment of: I love this!\n",
    options: ['schema' => ['enum' => ['positive', 'negative', 'neutral']]],
));
```

## The supported schema subset

The converter is strict by design: a keyword it doesn't implement
throws `InferException` naming the keyword. Silently ignoring a
constraint would hand you output that *looks* validated but isn't.

| Supported | Notes |
| --- | --- |
| `type: object` + `properties` | All properties are generated, in declaration order. `required`, when present, must list every property. |
| `type: array` + `items` | `minItems` of `0` (default) or `1`. |
| `type: string` / `integer` / `number` / `boolean` / `null` | JSON-strict lexical forms. |
| `enum` / `const` | Strings, numbers, booleans, null. |
| `anyOf` / `oneOf` | Compiled as alternation — `['anyOf' => [['type' => 'string'], ['type' => 'null']]]` is the nullable-field idiom. |
| `type: ["string", "null"]` | Multi-type shorthand, same alternation. |

Notably **not** supported (throws): `$ref`/`$defs`, optional
properties (a `required` list that's a proper subset), `pattern`,
`minLength`/`maxLength`, numeric ranges, `additionalProperties: true`
free-form objects, `minItems > 1`. Annotation-only keywords (`title`,
`description`, `default`, `examples`) are accepted and ignored.

## Raw GBNF (full control)

For shapes JSON Schema can't express, hand llama.cpp a
[GBNF grammar](https://github.com/ggml-org/llama.cpp/tree/master/grammars)
directly:

```php
$verdict = $model->raw(
    'Is the sky blue? Answer: ',
    maxTokens: 8,
    options: ['grammar' => 'root ::= "yes" | "no"'],
);
// $verdict is exactly "yes" or "no" — nothing else can be sampled
```

The grammar's start rule must be named `root`. `grammar` and `schema`
are mutually exclusive; passing both throws.

## How it interacts with everything else

- **Reasoning models** — the grammar applies from the first generated
  token, so a Qwen3-style model is *prevented* from opening a
  `<think>` block: it must start emitting the constrained shape
  immediately. For extraction tasks that's what you want.
- **`temperature`** — works as usual; sampling happens over the tokens
  the grammar allows. `0.0` (greedy) is the right default for
  extraction.
- **`finishReason()`** — once the grammar's root rule is fully
  matched, only end-of-generation remains legal, so completed
  constrained runs report `'eos'`. A run that hits `maxTokens` mid-
  structure reports `'length'` and the output is a truncated (invalid)
  document — size `maxTokens` generously.
- **Quality** — the grammar guarantees *shape*, not *truth*. A model
  too small for the task will fill your schema with confident nonsense.
  The schema is the seatbelt, not the driver.

## Errors

| Condition | Exception |
| --- | --- |
| Unsupported schema keyword | `InferException`, names the keyword |
| Schema not valid JSON / not array-or-string | `InferException` |
| `grammar` + `schema` together | `InferException` |
| GBNF llama.cpp can't parse | `InferException` |

See the [Options reference](./options.md) for the full option tables.
