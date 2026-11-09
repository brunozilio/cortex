---
name: release
description: Cut, check or repair a Superfik release (release-please PR, signed and notarized DMG on GitHub Releases). Use when asked to release, publish a version, check why a release failed, or rotate the signing certificate or notary key.
---

# Releasing Superfik

Releases are automatic. Nobody tags or uploads by hand.

```
squash-merge PRs to main (Conventional titles)
  └─ Release workflow: release-please updates the release PR (version + CHANGELOG.md)
merge the release PR
  └─ release-please tags vX.Y.Z and creates the GitHub release
       └─ publish job (macOS): build → sign (Developer ID) → notarize → staple → attach DMG + update zip
            └─ announce: sign the zip (update key) → attach superfik-<platform>.json to the release → installed apps update
       └─ publish-linux: .deb + .tar.gz → announce the tarball the same way
       └─ publish-server: superfik-server-X.Y.Z-linux-x86_64.tar.gz (what scripts/install-server.sh installs)
```

- Version lives in `Cargo.toml` `[workspace.package].version`; `.release-please-manifest.json` tracks the last release; `release-please-config.json` holds the setup. The lockfile job commits `Cargo.lock` to the release PR.
- Bumps come from PR titles (the squash commit): `feat:` minor, `fix:`/`perf:` patch, `feat!:` or a `BREAKING CHANGE:` footer major (minor while 0.x). `ci:`, `chore:`, `docs:`, `refactor:`, `test:` never release.
- `scripts/bundle-macos.sh` builds and signs; `SUPERFIK_RELEASE=1` makes it fail rather than ship unsigned or unnotarized.

## Cutting a release

1. `gh pr list --label "autorelease: pending"`: the open release PR. None means nothing releasable merged since the last tag.
2. Read its CHANGELOG diff with the user; edit the PR body or titles of merged PRs only if they ask.
3. Merge it when the user says so: `gh pr merge <n> --squash`.
4. Watch: `gh run list --workflow Release --limit 1`, then `gh run watch <id> --exit-status`. Notarization takes minutes, sometimes longer.
5. Confirm: `gh release view vX.Y.Z` lists `Superfik-X.Y.Z.dmg`, `Superfik-X.Y.Z-macos-aarch64.zip` and the Linux packages. Optionally download the DMG and run `spctl -a -vv -t install Superfik-X.Y.Z.dmg` (expect "Notarized Developer ID"). The "Announce the update" steps should end with `Announced X.Y.Z for <platform>`; a `::warning::` there means the update secrets are missing and installed apps will not see the release.

To force a version (e.g. the first 1.0.0), the user lands a commit whose body has `Release-As: 1.0.0`.

## When the publish job fails

Read `gh run view <id> --log-failed` and match:

| Symptom | Cause | Fix |
|---|---|---|
| `no 'Developer ID Application…' in the keychain` | `MACOS_CERTIFICATE` missing, wrong password, or expired | Rotate the certificate (below) |
| `notarytool store-credentials` fails | `APPLE_API_*` secrets wrong or key revoked | Rotate the API key (below) |
| notarization `Invalid` | Something in the bundle is unsigned or lacks hardened runtime | `xcrun notarytool log <submission-id>` locally with the same key; fix signing in `bundle-macos.sh` (inside out: framework, helpers, app) |
| `cmake`/`ninja` errors | CEF wrapper build | Check the `brew install` step |
| `SUPERFIK_UPDATE_KEY is not the key of superfik_update::UPDATE_KEY` | The secret is not the seed matching the built-in key | Set it again from `~/.superfik/update-signing-key.hex` (below) |
| announce `gh release upload` fails | The job's `GH_TOKEN` lacks `contents: write`, or the release is missing | Check the workflow's permissions and that release-please created the release |

Re-run after fixing secrets: `gh run rerun <id> --failed`. If the fix needs a code change, merge it as `fix:`; the next release PR carries it.

## Secrets (repository → Actions)

`MACOS_CERTIFICATE` (base64 .p12), `MACOS_CERTIFICATE_PASSWORD`, `APPLE_API_KEY` (base64 .p8), `APPLE_API_KEY_ID`, `APPLE_API_ISSUER`, `SUPERFIK_UPDATE_KEY`. Never print, echo or paste their values; set them only through `gh secret set` reading from a file or pipe.

- **Update key** (`SUPERFIK_UPDATE_KEY`): the Ed25519 seed (hex) that signs update archives; its public key is `superfik_update::UPDATE_KEY` in the app. It lives in `~/.superfik/update-signing-key.hex` (mode 0600, never committed) on Bruno's Mac. Set it with `gh secret set SUPERFIK_UPDATE_KEY < ~/.superfik/update-signing-key.hex`. Losing it means a new pair (`cargo run -p xtask -- update-keygen <file>`), a new `UPDATE_KEY`, and installed apps can only update by a manual download once.
- Repository variable (optional): `SUPERFIK_MINIMUM_VERSION` (versions below it must update; empty means none). Set with `gh variable set`.
- Without the update key the announce steps warn and skip; the release itself still publishes. Apps read `releases/latest/download/superfik-<platform>.json` (`SUPERFIK_RELEASES` points a build at a fork's releases).

- **Certificate** ("Developer ID Application: Bruno Zilio (F3BSQ3FZTF)") expires **2027-02-01**. Renew it in the Apple Developer portal before then, install it in the login keychain, and have the user run the export script (export the login keychain's only identity with `security export -t identities -f pkcs12` and a random password, then `gh secret set` both). The user must type the keychain password.
- **API key**: App Store Connect → Users and Access → Integrations → Team Keys, "Superfik CI" (Developer). It downloads once; if lost, revoke and create another, then set `APPLE_API_KEY` and `APPLE_API_KEY_ID`.

## Don'ts

- Don't push tags, create releases or upload DMGs by hand; release-please owns them.
- Don't sign or announce update archives by hand; the workflow does (a dispatch re-announces a failed one).
- Don't edit `CHANGELOG.md`, the manifest or the version outside the release PR.
- Don't merge the release PR without the user's go-ahead: it publishes.
