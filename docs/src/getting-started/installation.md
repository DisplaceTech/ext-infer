# Installation

Two supported install paths:

1. **[Via PIE](#via-pie)** — pulls a pre-built binary for your `(php-minor, arch, os, libc)` combo. No local C/C++ toolchain. Recommended for application developers.
2. **[From source](#from-source)** — builds llama.cpp locally via cargo. Needed for contributors, distros without a pre-built artifact, or anyone who wants to enable the `metal` cargo feature.

## Via PIE

> **Heads up:** PIE installation is wired up but the first published
> release (`v0.1.0`) is still in flight. Until then, install from
> source — the `pie install` flow becomes the recommended path the
> moment we ship binaries.

[PIE](https://github.com/php/pie) (PHP Installer for Extensions) is the
official tool for installing PHP extensions from Composer-style
metadata. Get it once:

```sh
curl -L --output pie.phar \
    https://github.com/php/pie/releases/latest/download/pie.phar
chmod +x pie.phar && sudo mv pie.phar /usr/local/bin/pie
```

Then install `ext-infer`:

```sh
pie install displace/ext-infer
```

PIE reads [`composer.json`](https://github.com/DisplaceTech/ext-infer/blob/main/composer.json)
to learn that `ext-infer` ships pre-packaged binaries, fetches the
right tarball from the matching [GitHub Release](https://github.com/DisplaceTech/ext-infer/releases),
extracts `infer.so` (or `infer.dylib` on macOS) into the PHP extension
directory, and adds it to your `php.ini`.

Verify the install with [`php -m`](./verifying.md):

```sh
php -m | grep infer
# infer
```

## From source

### Prerequisites

| Tool             | Purpose                                          | Minimum    |
| ---------------- | ------------------------------------------------ | ---------- |
| PHP CLI          | host process                                     | 8.3        |
| `php-config`     | tells `ext-php-rs` where the PHP headers are     | (matches PHP) |
| Rust toolchain   | compiles the extension                           | 1.88       |
| `cmake`          | llama.cpp builds via cmake during cargo build    | 3.18+      |
| C/C++ toolchain  | llama.cpp itself                                 | Clang / GCC |
| `cargo-php`      | wraps `make install` to drop the artifact in PHP's extension dir | 0.1+       |

The Rust toolchain is pinned via `rust-toolchain.toml`, so you don't
need to install a specific version manually — `rustup` will fetch it on
first build. On macOS, `cmake` is a `brew install cmake` away; on
Debian/Ubuntu, `apt install cmake build-essential libclang-dev`.

Install `cargo-php` once:

```sh
cargo install cargo-php
```

### Build and install

```sh
git clone https://github.com/DisplaceTech/ext-infer
cd ext-infer
make release          # builds target/release/libinfer.{so,dylib}
make install          # cargo php install --release
php -m | grep infer
```

A cold build compiles llama.cpp from source — that takes a few minutes
on a fresh machine. Subsequent builds reuse cargo's incremental cache
and the rebuilt llama.cpp object files; expect sub-minute rebuilds
after the first one.

### Without `make install` (development)

If you want to load a freshly built binary without committing to
installing it system-wide, pass the path on the PHP command line:

```sh
make build       # debug build (faster compile, slower runtime)
php -d extension=$PWD/target/debug/libinfer.dylib your-script.php
```

Substitute `.so` for `.dylib` on Linux. This is the workflow used
throughout the [examples](https://github.com/DisplaceTech/ext-infer/tree/main/examples).

### Apple Metal acceleration (opt-in)

The default build is CPU-only and portable. For Apple Silicon GPU
acceleration:

```sh
make release FEATURES=metal
make install  FEATURES=metal
```

See [Apple Metal](../advanced/metal.md) for what this does and what
trade-offs it implies.

## Uninstalling

Via PIE:

```sh
pie uninstall displace/ext-infer
```

From a source install:

```sh
make uninstall    # cargo php remove
```

Either way, confirm with `php -m | grep infer` (should produce no
output).

## Troubleshooting

If `php -m | grep infer` shows nothing after install, see
[Verifying your install](./verifying.md) for the diagnostic checklist —
it walks through the four or five most common failure modes
(`extension_dir` mismatch, PHP minor mismatch, missing
`-undefined,dynamic_lookup` on macOS, libc mismatch on Linux).
