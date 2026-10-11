# CI monorepo consolidation — 2026-10-11

## Plan

Account for all useful files in the unwanted repositories without changing them.
Keep all Cords configuration in `firebadnofire/cords`; prepare removal, never delete.

## Implementation

Canonical local commit: `c9cfa658c428f449ce60b20f61f1567260affb9a`.

| Scaffolding repository | Inspected HEAD | Complete tracked HEAD inventory | Disposition |
|---|---|---|---|
| `firebadnofire/cords-release-control` | `381b2205d856be015ef3b6be8dfb2a06398fc237` | 7 scripts, 4 public-pin/readme files, root README, `.gitattributes`, `.gitignore` | All 11 useful script/pin files byte-identical to canonical Git objects; root metadata describes the superseded design. |
| `firebadnofire/cords-publication` | `b90cb74451513d68bfe575e764dee85f1aa4f966` | Root README only | Superseded preparation description; no unique code/configuration. |

Controller helpers originated in Cords. No duplicate source or credential migration
is needed. The root README texts describe the former protected-controller and
publication-only proposals, both inactive pending security gates. Earlier creation
was authorized under the preceding design; the latest correction supersedes it.

## Validation

Reopened each repository's `settings/units` page: **Enable integrated CI/CD pipelines
with Forgejo Actions** is unchecked in both. No setting was saved in this unit.

```powershell
git -C C:/Users/william/AppData/Local/Temp/cords-release-control-20261011 ls-tree -r --name-only HEAD
git -C C:/Users/william/AppData/Local/Temp/cords-release-control-20261011 rev-parse HEAD
git ls-remote https://pubcode.archuser.org/firebadnofire/cords-publication.git
git clone --mirror --quiet https://pubcode.archuser.org/firebadnofire/cords-publication.git C:/Users/william/AppData/Local/Temp/cords-publication-20261011/inventory/mirrors/cords-publication.git
git -C C:/Users/william/AppData/Local/Temp/cords-publication-20261011/inventory/mirrors/cords-publication.git ls-tree -r --name-only HEAD
```

Python compared `git show HEAD:<path>` from each repository for all `scripts/`
and `.forgejo/release-keys/` files: **11/11 identical**, including exact GPG/Cosign
public pins. Git objects avoid CRLF working-copy conversion. No unique useful file
is missing from the canonical repository.

## Removal preparation

Both are redundant for the selected architecture; **neither has been deleted**.
Before separately approved removal, recheck refs, issues/PRs/releases/packages,
webhooks, deploy keys, runner bindings and dependencies; preserve a verified Git
bundle and metadata backup. This inventory covers tracked HEAD files and inactive
Actions, not every non-Git object. Deletion requires explicit user approval.
No secret, registration, host service, deployment or release-history change occurred.

Follow [the current monorepo plan](../ci-monorepo-plan.md).
