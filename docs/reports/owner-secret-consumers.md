# Owner-secret consumer inventory — 2026-10-11

## Plan

Inventory names and potential consumers without reading values or modifying secrets.
No credential migration is approved or performed.

## Implementation

Forgejo owner settings listed ten masked secret names: `CI_KEY`, `CI_KEY_B64`, `CI_KEY_PASSPHRASE`, `COSIGN_B64`, `COSIGN_PASSWORD`, `GH_KEY`, `KEY_ALIAS`, `KEY_PASSWORD`, `KEYSTORE_BASE64`, `KEYSTORE_PASSWORD`. 
All applicable owner secrets are included in ordinary owner-repository task payloads,
even if a workflow never references them. Repository separation does not prevent this.

Scanned 70 repositories from both owner listing pages plus the newly created
publication scaffolding repository: **71 repositories, 386 unique advertised commit trees**.
Bare mirrors were fetched/read with existing authentication and TLS verification;
no source checkout, workflow execution, secret-value request or credential export.

## Validation

```powershell
python scripts/test-inventory-owner-secrets.py
python scripts/inventory-owner-secrets.py --repositories C:/Users/william/AppData/Local/Temp/cords-publication-20261011/inventory/owner-repositories.json --directory C:/Users/william/AppData/Local/Temp/cords-publication-20261011/inventory/mirrors --output C:/Users/william/AppData/Local/Temp/cords-publication-20261011/inventory/owner-consumers.json
```

Both commands passed: five scanner fixtures and 70/70 repository ref-tree inventories.
The publication mirror was then scanned with the same `scan` implementation and
appended to the supplemental report. It has one README-only tree and no workflow.
Full names/paths/ref identities are retained locally in:
`C:/Users/william/AppData/Local/Temp/cords-publication-20261011/inventory/owner-consumers-71.json`.
Report SHA-256: `0974ebbba3ba37b56101e2e012491ccfacf6a7ee8e25581354d508fa0dc07d79`.

### Direct potential Forgejo consumers (heads and tags)

| Repository | Owner secret references in Forgejo/Gitea workflow snapshots |
|---|---|
| `firebadnofire/CentralSSH` | `GH_KEY` |
| `firebadnofire/Helium-sync` | `CI_KEY`, `CI_KEY_PASSPHRASE`, `GH_KEY` |
| `firebadnofire/VPNCTL` | `CI_KEY`, `CI_KEY_PASSPHRASE`, `GH_KEY` |
| `firebadnofire/WinTime` | `CI_KEY_B64`, `CI_KEY_PASSPHRASE`, `GH_KEY` |
| `firebadnofire/actions-template` | `GH_KEY` |
| `firebadnofire/cords` | `CI_KEY`, `CI_KEY_B64`, `CI_KEY_PASSPHRASE`, `COSIGN_B64`, `COSIGN_PASSWORD`, `GH_KEY` |
| `firebadnofire/google-wifi-led-control` | `CI_KEY`, `CI_KEY_PASSPHRASE`, `GH_KEY` |
| `firebadnofire/qbtOS` | `CI_KEY`, `CI_KEY_PASSPHRASE`, `GH_KEY` |
| `firebadnofire/rtsp-webview` | `CI_KEY`, `CI_KEY_PASSPHRASE`, `GH_KEY` |
| `firebadnofire/secure-dns-proxy` | `GH_KEY` |
| `firebadnofire/workflow-templates` | `GH_KEY`, `KEYSTORE_BASE64`, `KEYSTORE_PASSWORD`, `KEY_ALIAS`, `KEY_PASSWORD` |

These are potential consumers across current branch/tag snapshots, not proof of
active runtime usage. Historical tag references must not be silently discarded.
Other inherited recipients and GitHub workflow references are listed below.

### Complete repository coverage

| Repository | Unique ref trees | Workflow snapshots (including GitHub) | Skipped entries |
|---|---:|---:|---:|
| `firebadnofire/4chan-source-code` | 1 | 0 | 0 |
| `firebadnofire/AI-prompts` | 1 | 0 | 0 |
| `firebadnofire/Argon40case-Rust` | 1 | 0 | 0 |
| `firebadnofire/CentralSSH` | 67 | 67 | 1 |
| `firebadnofire/ComfyUI-JSON-to-PDF` | 1 | 0 | 0 |
| `firebadnofire/FreshWall` | 1 | 0 | 0 |
| `firebadnofire/FreshWall-wekv` | 41 | 15 | 20 |
| `firebadnofire/Helium-sync` | 14 | 14 | 0 |
| `firebadnofire/Label2CalCount` | 1 | 0 | 0 |
| `firebadnofire/README` | 1 | 0 | 0 |
| `firebadnofire/SimpleFolderLauncher` | 1 | 2 | 0 |
| `firebadnofire/VPNCTL` | 9 | 9 | 0 |
| `firebadnofire/WallpaperWorks` | 1 | 0 | 0 |
| `firebadnofire/Wikipedia-Donation-Remover` | 1 | 0 | 0 |
| `firebadnofire/WinTime` | 2 | 2 | 0 |
| `firebadnofire/WinZST` | 9 | 9 | 39 |
| `firebadnofire/actions-template` | 1 | 1 | 0 |
| `firebadnofire/addcad` | 1 | 0 | 0 |
| `firebadnofire/anubis-test` | 1 | 0 | 14 |
| `firebadnofire/appimage-updater` | 1 | 0 | 0 |
| `firebadnofire/apps-webpage` | 1 | 0 | 0 |
| `firebadnofire/audioscape` | 1 | 0 | 0 |
| `firebadnofire/audition` | 1 | 1 | 0 |
| `firebadnofire/authorized-ssh-keys` | 1 | 0 | 0 |
| `firebadnofire/automated-maintenance` | 1 | 0 | 0 |
| `firebadnofire/calcount-pwa` | 1 | 0 | 0 |
| `firebadnofire/central-ssh` | 1 | 0 | 0 |
| `firebadnofire/cloud-init-automation` | 1 | 0 | 0 |
| `firebadnofire/cloudflare-ddns` | 1 | 0 | 0 |
| `firebadnofire/cloudflare-ddns-rust` | 1 | 0 | 0 |
| `firebadnofire/cmdtime` | 1 | 0 | 0 |
| `firebadnofire/cords` | 5 | 9 | 0 |
| `firebadnofire/cords-release-control` | 1 | 0 | 0 |
| `firebadnofire/csgo-servers` | 1 | 0 | 0 |
| `firebadnofire/exponent-game` | 1 | 0 | 0 |
| `firebadnofire/faggots` | 1 | 0 | 0 |
| `firebadnofire/fileorg` | 1 | 0 | 0 |
| `firebadnofire/google-wifi-led-control` | 7 | 7 | 0 |
| `firebadnofire/horseshit` | 1 | 0 | 2 |
| `firebadnofire/java-launcher` | 1 | 0 | 0 |
| `firebadnofire/jsonlogs` | 1 | 0 | 0 |
| `firebadnofire/linux-appimage-updater` | 1 | 0 | 0 |
| `firebadnofire/local-inventory` | 1 | 0 | 0 |
| `firebadnofire/logdebarker` | 1 | 0 | 0 |
| `firebadnofire/mouseover-windows` | 1 | 0 | 0 |
| `firebadnofire/mqtt-docker` | 1 | 0 | 0 |
| `firebadnofire/mullvad-browser` | 0 | 0 | 0 |
| `firebadnofire/muse` | 1 | 1 | 0 |
| `firebadnofire/ollama-client` | 1 | 0 | 0 |
| `firebadnofire/openvdp` | 1 | 0 | 0 |
| `firebadnofire/ostree-resolver` | 1 | 0 | 0 |
| `firebadnofire/php-holidays` | 1 | 0 | 0 |
| `firebadnofire/qbtOS` | 33 | 35 | 65 |
| `firebadnofire/quick-search-bar` | 1 | 0 | 0 |
| `firebadnofire/rms` | 1 | 0 | 0 |
| `firebadnofire/rollcat` | 1 | 0 | 0 |
| `firebadnofire/rtsp-webview` | 33 | 33 | 0 |
| `firebadnofire/scale-code` | 4 | 0 | 2 |
| `firebadnofire/secure-dns-proxy` | 26 | 17 | 26 |
| `firebadnofire/shadowsocks-install` | 1 | 0 | 0 |
| `firebadnofire/torrent-webui` | 1 | 0 | 1 |
| `firebadnofire/trapmaster` | 1 | 0 | 0 |
| `firebadnofire/upload-artifact` | 79 | 393 | 86 |
| `firebadnofire/usb-syncd` | 1 | 0 | 0 |
| `firebadnofire/vdd` | 1 | 0 | 0 |
| `firebadnofire/voxa` | 1 | 0 | 0 |
| `firebadnofire/wheel` | 1 | 0 | 0 |
| `firebadnofire/winget-GUI` | 1 | 0 | 0 |
| `firebadnofire/workflow-templates` | 1 | 4 | 0 |
| `firebadnofire/zshrc` | 1 | 0 | 0 |
| `firebadnofire/cords-publication` | 1 | 0 | 0 |

## Migration review and unresolved consumers

No secret can yet be classified as safe to delete or relocate from owner scope.
Cords private signing, registry and GitHub credentials must ultimately be provisioned
outside Actions in the independently protected executor, after explicit approval.
Other directly referencing projects above require consumer-specific restricted
provisioning and validation before removal of any shared owner entry. Their security
boundaries cannot be inferred from a matching secret name. Do not change unrelated
projects or assume `CI_KEY` is encoded like `CI_KEY_B64`.

Remaining required review: repository-secret overrides and enabled Actions units;
external/composite actions and dynamically selected secret names; environment-only
consumers and non-workflow scripts; oversized blobs/submodules; queued jobs/deleted
historical refs; ownership changes and consumers outside this instance. The scanner
reports static syntax and paths only, cannot prove execution-time secret consumption,
and does not inspect values. Some comments/doc references are conservative positives.
All applicable ordinary workflows inherit every owner secret regardless of this table.

Before migration, produce an approved per-project/per-secret destination mapping,
validate consumers with least privilege and recovery copies, drain affected jobs,
provision replacement scope without printing values, verify each consumer, then remove
owner exposure only under separate authorization. Preserve intentionally shared public
identities. Never restore broad exposure automatically to make a failed job green.

Until this review and enforceable runner isolation pass, expanded ordinary CI and
privileged publication remain blocked. This is a complete advertised-ref inventory
with explicit consumer-analysis limitations, not a completed secret migration.
