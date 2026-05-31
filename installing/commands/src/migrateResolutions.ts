import path from 'node:path'

import { confirm } from '@inquirer/prompts'
import { convertResolutionsToOverrides } from '@pnpm/config.parse-overrides'
import { globalInfo, logger } from '@pnpm/logger'
import type { ProjectManifest } from '@pnpm/types'
import { writeProjectManifest } from '@pnpm/workspace.project-manifest-writer'
import { updateWorkspaceManifest } from '@pnpm/workspace.workspace-manifest-writer'
import { isCI } from 'ci-info'

export interface MigrateResolutionsOptions {
  rootProjectManifest: ProjectManifest
  rootProjectManifestDir: string
  workspaceDir: string | undefined
  overrides: Record<string, string> | undefined
  save: boolean | undefined
}

export async function maybeMigrateResolutions (opts: MigrateResolutionsOptions): Promise<Record<string, string> | undefined> {
  if (opts.save === false) return undefined
  if (!opts.rootProjectManifest.resolutions) return undefined

  const hasExistingOverrides = opts.overrides != null && Object.keys(opts.overrides).length > 0
  if (hasExistingOverrides) return undefined

  const canPrompt = !isCI && Boolean(process.stdin.isTTY)

  const { overrides: converted, skipped } = convertResolutionsToOverrides(
    opts.rootProjectManifest.resolutions
  )

  if (Object.keys(converted).length === 0 && skipped.length === 0) return undefined

  if (skipped.length > 0) {
    for (const { selector, reason } of skipped) {
      logger.warn({
        message: `Cannot auto-migrate resolution "${selector}": ${reason}`,
        prefix: opts.rootProjectManifestDir,
      })
    }
  }

  if (Object.keys(converted).length === 0) return undefined

  if (!canPrompt) {
    globalInfo(
      `${Object.keys(converted).length} resolution(s) in package.json could be migrated to overrides in pnpm-workspace.yaml. Re-run in an interactive terminal to proceed.`
    )
    return undefined
  }

  const answer = await confirm({
    message: `Migrate ${Object.keys(converted).length} resolution(s) from package.json to overrides in pnpm-workspace.yaml?`,
    default: true,
  })

  if (!answer) return undefined

  const workspaceDir = opts.workspaceDir ?? opts.rootProjectManifestDir
  await updateWorkspaceManifest(workspaceDir, {
    updatedOverrides: converted,
  })

  const { resolutions: _resolutions, ...manifestWithoutResolutions } = opts.rootProjectManifest
  const manifestPath = path.join(opts.rootProjectManifestDir, 'package.json')
  await writeProjectManifest(manifestPath, manifestWithoutResolutions)

  globalInfo(`Migrated ${Object.keys(converted).length} resolution(s) to overrides in pnpm-workspace.yaml`)

  return converted
}
