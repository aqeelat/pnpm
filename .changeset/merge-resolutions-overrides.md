---
"@pnpm/config.reader": patch
"pnpm": patch
---

Fix handling of `resolutions` in root `package.json` when `overrides` is set in `pnpm-workspace.yaml`. When both exist, `resolutions` is now ignored and a warning is printed. When only `resolutions` exists, it is used as overrides with a deprecation warning recommending `overrides` in `pnpm-workspace.yaml` instead.
