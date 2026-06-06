# Releasing

The full cut-a-release process lives in
[`RELEASE.md`](https://github.com/DisplaceTech/ext-infer/blob/main/RELEASE.md)
at the repo root. This page is the one-screen version with pointers
back into that document.

## The five-step shape

```sh
# 1. Bump versions
edit Cargo.toml                      # [package].version = "0.1.0"

# 2. Verify locally
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
INFER_TEST_MODEL=$PWD/models/Qwen3-0.6B-Q8_0.gguf make test
composer validate composer.json

# 3. Land the bump
git commit -am "chore(release): v0.1.0"
git push

# 4. Tag — this is what triggers the release workflow
git tag v0.1.0
git push --tags

# 5. Edit and publish the draft Release on GitHub
```

Step 4 is the only user-facing action. The
[release workflow](https://github.com/DisplaceTech/ext-infer/blob/main/.github/workflows/release.yml)
takes it from there.

## What the workflow does

For each `(PHP minor, OS, arch)` in the 9-leg matrix:

1. Install system deps (`cmake`, `build-essential`, …).
2. Install the matrix PHP via `shivammathur/setup-php@v2`.
3. `cargo build --release`.
4. Stage `infer.so` / `infer.dylib` in the right shape.
5. Tarball as `php_infer-{version}_php{minor}-{arch}-{os}[-{libc}].tar.gz`
   per [PIE's filename convention](https://github.com/php/pie/blob/1.5.x/docs/extension-maintainers.md).
6. Compute a `.sha256` sidecar.
7. Upload both to a **draft** GitHub Release.

The first matrix leg creates the draft Release; later legs add files
to the same one.

## Why "draft"?

Releases ship draft so a maintainer can:

- Verify all 18 files (9 tarballs + 9 sidecars) are attached.
- Write release notes — the workflow doesn't auto-generate them.
- Spot-check one tarball locally with `pie install` before exposing it
  to users.

After the manual review, hit **Publish release** in the GitHub UI.

## Versioning policy

Pre-1.0 (`0.x.y`), breaking changes happen between minors (`0.1` →
`0.2`), not patches. Once `v1.0.0` ships, the class / method / argument
surface is frozen.

`composer.json` does NOT carry a version key — that would conflict
with the tag-derived version Composer infers. The `branch-alias` under
`extra` exists only so `dev-main` resolves to `0.1.x-dev` for users
pinning a dev branch.

## What [`RELEASE.md`](https://github.com/DisplaceTech/ext-infer/blob/main/RELEASE.md) covers in more detail

- Pre-flight checklist (the verify-locally step expanded).
- Release-notes template.
- Post-publish smoke test (install via PIE, run hello-world).
- Hotfix / patch process.
- Yanking a broken release.
- Caveats (Windows excluded, ZTS untested, etc.).
- Symptom → first-thing-to-check table for release failures.

If you're cutting a release, **read `RELEASE.md` first**. This page is
the index, not the manual.

## Caveats

Three things v0.1 explicitly doesn't ship and that you should know
about before cutting one:

- **No Windows binaries.** `os-families-exclude: ["windows"]` in
  `composer.json` makes PIE skip Windows hosts cleanly.
- **No ZTS binaries.** The composer.json declares `support-zts: true`
  because the code is thread-safe by construction, but the release
  matrix doesn't include a ZTS runner. ZTS users need to build from
  source for now.
- **No musl Linux binaries.** The release matrix is glibc only.
  Musl users build from source; the `.cargo/config.toml` carries the
  needed `crt-static` opt-out.

All three are tracked in
[`PLAN.md`](https://github.com/DisplaceTech/ext-infer/blob/main/PLAN.md).

## Next

- [`RELEASE.md`](https://github.com/DisplaceTech/ext-infer/blob/main/RELEASE.md)
  — the full process document.
- [`PLAN.md`](https://github.com/DisplaceTech/ext-infer/blob/main/PLAN.md)
  — what's in flight after v0.1.
