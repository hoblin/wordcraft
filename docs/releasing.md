# Releasing WordCraft (this fork)

This fork (`hoblin/wordcraft`) is maintained for Bondata's own use, so it releases one build, Linux
x86_64, from `.github/workflows/release.yml`; upstream's other workflows are removed.

1. Tag the commit on `main` as `v<upstream version>-bondata.<n>`, e.g. `v0.6.1-bondata.2`, and push
   the tag. `[workspace.package] version` in `Cargo.toml` stays at the upstream version on `main`;
   the workflow sets it from the tag (`cargo xtask version set`) before building, so the archive
   name, the metainfo and `--version` all carry `0.6.1-bondata.2`.
2. The push runs `packaging/linux/package.sh --formats tar` on `ubuntu-22.04` (glibc 2.35, so the
   binaries run on the studio image's Debian bookworm, glibc 2.36) with the fonts from
   `storytold/craft-fonts` embedded, and publishes a GitHub release `WordCraft v<version>` with:

| File | Contents |
|---|---|
| `wordcraft-<version>-linux-x86_64.tar.gz` | upstream's Linux tarball shape: `wordcraft-<version>-linux-x86_64/{bin/wordcraft,bin/wordcraft-cli,share/...}` |
| `SHA256SUMS.txt` | `sha256sum` of every file |

A consumer downloads it the way it downloaded upstream's tarball, from
`https://github.com/hoblin/wordcraft/releases/download/v<version>/`.

A failed build is re-run from the Actions page. A tag that already has a release needs a new tag.

Local packaging: `packaging/linux/package.sh` (every format; add `--formats tar` for the tarball alone).
