---
"@pnpm/config.parse-overrides": minor
"@pnpm/installing.commands": minor
"pnpm": minor
---

Auto-migrate `resolutions` from `package.json` to `overrides` in `pnpm-workspace.yaml` during `pnpm install`. On first install, if `resolutions` exist in `package.json` and no `overrides` are set, pnpm prompts to migrate them — converting Yarn-style selectors to pnpm override selectors, writing `overrides` to `pnpm-workspace.yaml`, and removing `resolutions` from `package.json`. Unconvertible patterns (glob `**`, Berry qualifiers) are skipped with warnings. Non-interactive/CI environments show an informational message and skip [#12066](https://github.com/pnpm/pnpm/issues/12066).
