# Releasing

1. Bump `version` in `Cargo.toml` and run `cargo build` so `Cargo.lock` follows
2. Turn `## Unreleased` in `CHANGELOG.md` into `## vX.Y.Z (YYYY-MM-DD)`
3. Commit (`release vX.Y.Z`), tag and push:

```bash
git tag vX.Y.Z
git push origin main vX.Y.Z
```

Pushing the tag runs `.github/workflows/release.yml`, which:

1. Builds binaries for macOS (arm64, x86_64), Linux and Windows
2. Publishes the crate to crates.io, once every build has succeeded. It fails
   if the tag does not match the `Cargo.toml` version, and skips publishing if
   that version is already on crates.io, so re-running the workflow is safe
3. Creates the GitHub release with the binaries attached

crates.io publishing uses [trusted publishing](https://crates.io/docs/trusted-publishing),
so no API token is stored in the repository. The crate's settings on crates.io
list this repository and workflow (`chuilishi/claude-history`, `release.yml`)
as a trusted publisher.

A published version cannot be deleted from crates.io, only yanked
(`cargo yank --version X.Y.Z`).
