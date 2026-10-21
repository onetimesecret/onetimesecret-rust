# Releasing

How this SDK is set up for publishing and how a release is cut. The
workflow has the same shape across the Onetime Secret SDKs
(`onetimesecret-ruby`, `onetimesecret-py`, `onetimesecret-js`): the version lives in
the repository, pushing a `vX.Y.Z` tag publishes it, CI verifies the tag
matches the package version, the registry trusts the workflow through OIDC
rather than a stored token, and a GitHub Release is created from the
changelog. The language-specific parts are the registry and the version
file.

## Names

| What | Value |
| --- | --- |
| Repository | `onetimesecret/onetimesecret-rust` |
| Crate (crates.io) | `onetimesecret` |
| Version source | `Cargo.toml` → `[package] version` |
| Changelog | `CHANGELOG.md`, newest `## [x.y.z] - YYYY-MM-DD` heading |
| Workflow | `.github/workflows/release.yml` |
| GitHub environment | `crates-io` |
| Documentation | <https://docs.rs/onetimesecret>, built by docs.rs on publish |

Repository and crate names do not need to match; the Ruby SDK follows the
same pattern (`onetimesecret-ruby` / `onetimesecret`). `onetime` and `ots` on
crates.io belong to unrelated crates.

## Where the version lives

Two places must agree, and CI refuses to publish when they don't:

1. `Cargo.toml` → `version`
2. `CHANGELOG.md` → the newest `## [x.y.z] - YYYY-MM-DD` heading

`scripts/check-release.sh` verifies them (and the tag, when given one). It
runs in CI on every push and pull request, and again in the Release
workflow with the tag.

`VERSION` in the code is read from `Cargo.toml` at compile time and goes
out in the `User-Agent` and `X-Onetime-Client` headers.

## One-time setup

Do these before the first tag.

### 1. First publish, by hand

crates.io trusted publishing can only publish to a crate that already
exists, so version 0.1.0 is published from a workstation by someone who
will own the crate:

```sh
cargo login                       # a crates.io token with publish-new scope
scripts/check-release.sh          # version and changelog agree
cargo publish --dry-run
cargo publish
```

Then add owners so the crate does not hinge on one account:

```sh
cargo owner --add github:onetimesecret:maintainers   # a GitHub team
cargo owner --add <other-maintainer>
```

Tag what was published and create the release so the history starts
clean (the workflow will do this for every later version):

```sh
git tag -a v0.1.0 -m v0.1.0 && git push origin v0.1.0
```

Pushing the tag runs the Release workflow. Its publish step is a no-op
error ("crate version already uploaded") until step 2 exists; re-run the
workflow after step 2, or create the 0.1.0 GitHub Release by hand with
`gh release create v0.1.0 --notes-file <(scripts/changelog-section.sh 0.1.0)`.

### 2. crates.io: trusted publisher

On <https://crates.io/crates/onetimesecret/settings>, under **Trusted
Publishing**, add a GitHub publisher:

| Field | Value |
| --- | --- |
| Repository owner | `onetimesecret` |
| Repository name | `onetimesecret-rust` |
| Workflow filename | `release.yml` |
| Environment | `crates-io` |

Every field must match exactly; crates.io compares them with the OIDC
token's claims. The `publish` job requests `id-token: write` and
`rust-lang/crates-io-auth-action` exchanges it for a 30-minute publish
token. No long-lived crates.io token is stored anywhere.

### 3. GitHub environment

In **Settings → Environments → `crates-io`** (the workflow creates it,
unprotected, on first run; configure it before releasing):

- Deployment branches and tags: tags matching `v*` only.
- Required reviewers: add one or more when a human gate before publishing
  is wanted. Leave empty for tag-push-to-publish.

### 4. Repository settings

- **Pull requests**: squash merge only, PR title as the commit message.
- **Actions → General → Workflow permissions**: read-only is enough; the
  jobs that write declare their own permissions.
- **Rules → `main`**: require a pull request and the `rustfmt`, `clippy`,
  `test (ubuntu-latest, stable)`, `MSRV (lib)`, `rustdoc`, `cargo-deny`
  and `version and changelog agree` checks; block force pushes; apply to
  admins.
- **Rules → tags `v*`**: restrict creation to maintainers, since creating
  a tag publishes.
- **Code security**: Dependabot alerts and security updates on. Version
  updates (crates weekly, actions weekly) are in `.github/dependabot.yml`;
  the actions group keeps the SHA pins current.
- `.github/CODEOWNERS` names the maintainer; adjust it.

## Cutting a release

1. Make sure `main` is green.
2. On a branch, set the new version in `Cargo.toml` and move the
   `## [Unreleased]` notes (if any) under a new
   `## [x.y.z] - YYYY-MM-DD` heading in `CHANGELOG.md`. Run
   `scripts/check-release.sh vx.y.z` locally. For a breaking change,
   `cargo semver-checks` says whether the bump is big enough.
3. Open a pull request titled `chore: release vx.y.z`; CI runs. Merge it.
4. Tag the merge commit and push the tag:

   ```sh
   git switch main && git pull
   git tag -a vx.y.z -m vx.y.z
   git push origin vx.y.z
   ```

5. The Release workflow runs: `verify` (tag, formatting, clippy, tests,
   `cargo publish --dry-run`), `publish` (crates.io), `github-release`
   (notes from the changelog section). Watch it in the Actions tab.
6. Check <https://docs.rs/onetimesecret> once docs.rs has built it. A
   docs.rs failure is the one thing CI cannot catch beforehand, which is
   why the `rustdoc` CI job builds with `--cfg docsrs` on nightly.

Version rules, pre-1.0: a breaking change bumps the minor version; an
addition or fix bumps the patch version. From 1.0, SemVer as usual.
`cargo-semver-checks` runs on every pull request and refuses an
undeclared breaking change. It compares against the newest version on
crates.io, so CI skips it until 0.1.0 is published.

## If something goes wrong

- **The `verify` job fails.** Nothing was published. Fix on `main`,
  delete the tag locally and remotely (`git push --delete origin vx.y.z`),
  and tag again.
- **`publish` fails after `verify` passed.** Usually the trusted-publisher
  record or environment name does not match. Fix the record and re-run
  the failed jobs from the Actions tab; the tag stays.
- **A published version is broken.** Yank it and fix forward with a patch
  release:

  ```sh
  cargo yank --version x.y.z
  ```

  Yanking does not delete the version; projects with it in `Cargo.lock`
  keep building, new resolutions skip it.

- **A pre-release** (`0.5.0-rc.1`) follows the same steps; crates.io
  treats it as a pre-release and `cargo add` will not pick it by default.
