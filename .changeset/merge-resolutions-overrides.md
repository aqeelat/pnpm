---
"@pnpm/config.reader": minor
"pnpm": minor
---

Fix handling of `resolutions` in root `package.json` when `overrides` is set in `pnpm-workspace.yaml`. When both exist, pnpm now errors by default. Pass `--ignore-resolutions-conflict` or set `ignoreResolutionsConflict: true` in `pnpm-workspace.yaml` to suppress the error and use `overrides` only. When only `resolutions` exists, it is used as `overrides` with a deprecation warning.
