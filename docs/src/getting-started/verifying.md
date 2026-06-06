# Verifying your install

After installing, three things should be true. If any of them isn't,
this page is the checklist.

## The fast version

```sh
# 1. Is the extension loaded?
php -m | grep infer
# expected: infer

# 2. Are the classes registered?
php -r 'echo class_exists("Displace\\Infer\\Model") ? "yes\n" : "no\n";'
# expected: yes

# 3. Does inference actually work?
php -r '
$m = \Displace\Infer\Model::load("models/Qwen3-0.6B-Q8_0.gguf");
$r = $m->chat(\Displace\Infer\Prompt::user("Say hello."));
echo $r->answer(), PHP_EOL;
$m->close();
'
# expected: a one-line greeting
```

All three pass → you're done. Skip to the [Guide](../guide/prompts.md).

## Diagnosis if `php -m | grep infer` is empty

The extension didn't load. PHP loads extensions from a specific
directory and looks for them by exact filename — usually one of these
four things is off.

### 1. PHP can't find the binary

Confirm where PHP is looking:

```sh
php -i | grep -E '^extension_dir|^Loaded Configuration File'
```

Then confirm the binary is in that directory:

```sh
ls -l $(php -r 'echo ini_get("extension_dir");')/infer.*
```

If the file is missing:

- After `make install`, `cargo-php` should have placed it there. Try
  re-running with `-v` to see where it landed:
  `make install` (or `cargo php install --release -v`).
- After `pie install`, look at PIE's output for the install path.

If the file is in a *different* directory than `extension_dir`, either
move it or update `extension_dir` in your `php.ini`.

### 2. PHP minor mismatch

A binary built against PHP 8.4 will not load into PHP 8.5 (and vice
versa). Confirm both:

```sh
php --version | head -1
# e.g. PHP 8.4.20 (cli)

# For PIE-installed binaries, the tarball filename encodes the PHP
# minor — check the GitHub Release you installed from:
ls -l $(php -r 'echo ini_get("extension_dir");')/infer.*
```

Cross-check that the binary's PHP minor matches your running PHP minor.
If they disagree, re-install with the right tarball (PIE handles this
automatically; manual installs may need `pie install --force`).

### 3. macOS: `-undefined dynamic_lookup` missing from the link

The extension uses `dlopen`-style undefined-symbol resolution against
the host PHP binary. If you built from source on macOS and skipped the
extension's own `build.rs`, the linker errors out at build time with
`Undefined symbols for architecture arm64`. From-source builds via
`make build` / `make release` configure this automatically. If you
invoked `cargo build` from somewhere unusual (e.g. an IDE), repeat the
build via `make` to be safe.

### 4. Linux: libc mismatch

The released binaries target glibc Linux. Alpine (musl) is **not** in
the v0.1 release matrix. Confirm your libc:

```sh
ldd --version 2>&1 | head -1
# expected: ldd (GNU libc) 2.x
# if you see musl: rebuild from source — see Installation
```

Building from source on musl works; `.cargo/config.toml` carries the
needed `crt-static` opt-out.

## Diagnosis if classes are missing

If `php -m` shows `infer` but `class_exists("Displace\\Infer\\Model")`
returns `no`, the namespace probably has a typo somewhere upstream of
you. The full list:

```
Displace\Infer\Model
Displace\Infer\Prompt
Displace\Infer\Message
Displace\Infer\Response
Displace\Infer\Embedding
Displace\Infer\InferException
Displace\Infer\ModelLoadException
Displace\Infer\InferenceException
```

All eight should exist after a successful load. If only some do, you
likely have a `ext-infer` install left over from an older API surface —
uninstall the old version (`pie uninstall` or `make uninstall`) and
reinstall.

## Diagnosis if inference fails

If `Model::load` throws `ModelLoadException`:

- **"no such file"** — the GGUF path is wrong. PHP resolves relative
  paths against the working directory, not the script's directory.
- **"failed to load model: …"** — check that the file isn't truncated
  (`du -h` should match what the publisher lists) and that it really is
  a GGUF (`file <path>` should mention "data" or similar; if it says
  "ASCII text" it's probably an HTML 404 from a failed download).

If `Model::chat` throws `InferenceException` with `"model has no
embedded chat template"`, you've picked a base model rather than an
instruct/chat variant. See [Choosing a model](../guide/models.md) or
use [`Model::raw()`](../guide/raw.md) with your own templating.

If the script segfaults rather than throwing — please open an issue at
[github.com/DisplaceTech/ext-infer/issues](https://github.com/DisplaceTech/ext-infer/issues)
with the model name, PHP version, and OS. That's a bug.

## Enabling verbose logging

llama.cpp's own diagnostic chatter is silenced by default. To see it
(model layout, KV cache sizing, graph reservation, ...):

```sh
EXT_INFER_LOG=1 php hello.php
```

A noisy log can sometimes point straight at the issue — e.g. "n_ctx
exceeds model's training context" tells you the model is being asked to
handle longer input than it was trained for.
