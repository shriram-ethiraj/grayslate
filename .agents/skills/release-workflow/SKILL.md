---
name: release-workflow
description: Prepare, verify, tag, monitor, and recover Grayslate releases. Use for version bumps, release PRs, v-prefixed Git tags, failed release workflows, or any request to publish the next Grayslate version.
---

# Grayslate Release Workflow

Use the repository-owned scripts as the source of truth. Never edit release version fields individually unless repairing the script itself.

## Prepare the version bump

1. Confirm the worktree is clean and refresh `main` and tags from `origin`.
2. Fast-forward local `main`; never tag a stale local commit.
3. Create a branch named `chore/bump-version-X.Y.Z`.
4. Run:

   ```bash
   pnpm release:version X.Y.Z --date YYYY-MM-DD
   node scripts/release/verify-release.mjs vX.Y.Z
   pnpm run test:release-scripts
   ```

   The bump command updates `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, the Grayslate entry in `Cargo.lock`, and Linux AppStream metadata.

5. Run `git diff --check`, `pnpm run check`, and `cargo test --manifest-path src-tauri/Cargo.toml`.
6. Review and stage only the intended release files.
7. Obey the repository's no-auto-commit rule. Ask the developer to commit the staged changes manually, then resume the workflow.

## Raise and merge the PR

After the developer's commit exists:

1. Push the bump branch and open a PR against `main`.
2. Report the exact checks and diff in the PR body.
3. Wait for required checks to pass, then merge using the repository's normal merge method.
4. Refresh and fast-forward local `main` to the merged commit.

Do not tag the pre-merge branch or assume that opening a PR authorizes bypassing required checks.

## Create the release tag

1. Re-run `node scripts/release/verify-release.mjs vX.Y.Z` on the updated `main`.
2. Confirm the tag does not already exist locally or remotely.
3. Follow the existing annotated-tag convention:

   ```bash
   git tag -a vX.Y.Z -m "Grayslate X.Y.Z"
   git push origin refs/tags/vX.Y.Z
   ```

4. Verify the remote tag resolves to the intended `main` commit.
5. Monitor the `Build draft release` workflow through completion and report failures with the failing step and exact error.

## Recover a prematurely created tag

If a release tag was pushed before the version bump merged, rerunning the workflow cannot fix it because the tag still points to the old commit.

1. Obtain explicit approval to delete the local and remote tag.
2. Delete the failed tag instead of force-moving it.
3. Complete and merge the version-bump PR.
4. Recreate the annotated tag on updated `main`, push it, and monitor the new release run.

Never delete, replace, or force-push a release tag without confirming its exact target and receiving explicit user authorization.
