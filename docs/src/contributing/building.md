# Building from source

The development build is what `make build` produces — a debug-mode
shared library you can load via `-d extension=…`. The release build
is what ships in PIE tarballs.

## Prerequisites

- **PHP 8.3+** with `php-config` on `PATH`.
- **Rust** — installed via [rustup](https://rustup.rs/). The repo
  pins the toolchain via `rust-toolchain.toml`; `rustup` will fetch
  it on first build.
- **cmake 3.18+** — llama.cpp's build system.
- **A C/C++ compiler** — Clang (macOS / Linux) or GCC. The build
  script honors `CC` / `CXX` if you need to override.
- **libclang** (Linux only) — `apt install libclang-dev` or distro
  equivalent. Used by `bindgen` for the PHP header parse.
- **`cargo-php`** — `cargo install cargo-php` once.

Verify everything:

```sh
php --version
php-config --version
rustup --version
cmake --version
cargo php --version
```

## Cloning

```sh
git clone https://github.com/DisplaceTech/ext-infer
cd ext-infer
```

The repo includes a `models/` directory (gitignored) where you can
drop GGUFs for testing. The PHPT suite and examples both default to
`models/Qwen3-0.6B-Q8_0.gguf`.

## Debug build

```sh
make build
# -> target/debug/libinfer.{so,dylib}
```

Debug builds compile faster but run slower. Use them for iterative
development; switch to `make release` when you're benchmarking or
shipping.

A cold `make build` takes a few minutes because cargo compiles
`llama-cpp-sys-2` from source (it vendors all of llama.cpp). Cached
incremental rebuilds are sub-minute on a modern laptop.

## Release build

```sh
make release
# -> target/release/libinfer.{so,dylib}
```

Use this for installing system-wide via `make install`, for the
performance numbers you'd quote in benchmarks, and for any
"production-like" testing.

### Optional features

| Feature | Effect                                                  | When to use                                                       |
| ------- | ------------------------------------------------------- | ----------------------------------------------------------------- |
| `metal` | Enables Apple Metal GPU offload on macOS-arm64.         | When you have an Apple Silicon Mac and want GPU acceleration. See [Apple Metal](../advanced/metal.md). |

```sh
make release FEATURES=metal
```

## Loading your build into PHP

Two options.

### Without installing

Pass `-d extension=…` on every PHP invocation:

```sh
php -d extension=$PWD/target/debug/libinfer.dylib your-script.php
```

Substitute `.so` on Linux. This is what every script in
[`examples/`](https://github.com/DisplaceTech/ext-infer/tree/main/examples)
assumes — you can drop the flag once you `make install`.

### Installing system-wide

`make install` runs `cargo php install --release`, which:

1. Builds release-mode if it hasn't already.
2. Drops the binary into PHP's `extension_dir`.
3. Adds `extension=infer.so` (or `.dylib`) to a config file in
   `php.ini`'s scan directory.

```sh
make install
php -m | grep infer
# infer
```

To revert:

```sh
make uninstall
```

## Editor / IDE setup

### Rust analyzer

The Rust code lives in `src/`. Pointing `rust-analyzer` at
`Cargo.toml` (default) Just Works.

### PHP autocomplete

Use the hand-authored stubs at `stubs/infer.stubs.php`:

```jsonc
// .phpstorm.meta.php / .composer.json autoload config:
{
  "autoload-dev": {
    "files": ["stubs/infer.stubs.php"]
  }
}
```

Or symlink it into your project. The stubs include full PHPDoc on
every method so hovering in your IDE shows the option semantics
without flipping to the docs.

## Regenerating stubs (rare)

Stubs are hand-authored today because we want richer docblocks than
`cargo php stubs` emits. To regenerate from scratch (e.g. to confirm
the stub signatures match what's actually registered):

```sh
make stubs
git diff stubs/infer.stubs.php
```

Reconcile the generated output with the hand-authored version
manually.

## Troubleshooting common build failures

| Error                                          | Likely fix                                                    |
| ---------------------------------------------- | ------------------------------------------------------------- |
| `linker 'cc' not found` / `cc: command not found` | Install Xcode CLT (`xcode-select --install`) or `build-essential` (Ubuntu). |
| `cmake: command not found`                     | `brew install cmake` or `apt install cmake`.                  |
| `libclang.so: cannot open shared object`       | `apt install libclang-dev` (Linux). On macOS, libclang comes with the CLT. |
| `php-config: command not found`                | Install PHP CLI; on macOS via Homebrew use `brew link php@8.4 --force`. |
| `cargo install cargo-php` fails                | Check your Rust version. `rustup update` may help.            |
| `undefined symbol: _spl_ce_RuntimeException`   | The dynamic-lookup link flag didn't apply. Check `build.rs` ran; usually a stale `target/` — `cargo clean` and rebuild. |

## Next

- [Testing](./testing.md) — running PHPT and Rust unit tests.
- [Releasing](./releasing.md) — cut-a-release process.
