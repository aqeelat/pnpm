import type { LockfileObject, PackageSnapshot } from '@pnpm/lockfile.fs'
import { nameVerFromPkgSnapshot } from '@pnpm/lockfile.utils'
import type { DepPath, ProjectId, ProjectManifest } from '@pnpm/types'
import semver from 'semver'

/**
 * Walk the resolved lockfile to determine which override selectors matched
 * at least one dependency. Used on the pnpr-server path where the resolver
 * runs server-side and does not report applied selectors back.
 *
 * A selector is considered matched if its target name appears as a
 * dependency key in any importer or package snapshot. The target's
 * version range (bareSpecifier) is NOT checked against the resolved
 * version because the lockfile stores post-override values — the
 * override already changed the version, so comparing the new version
 * against the old selector range would produce false positives (e.g.
 * `foo@^1: 2.0.0` resolves to 2.0.0, which doesn't satisfy ^1).
 * Parent-scoped selectors check both resolved packages and workspace
 * project manifests (importers) for parent identity.
 *
 * `projectManifests` maps importer IDs to the workspace project's
 * manifest, so parent-scoped overrides can match project names that
 * don't appear in `lockfile.packages`.
 */
export function findAppliedOverrideSelectorsFromLockfile (
  lockfile: LockfileObject,
  parsedOverrides: Array<{ selector: string, parentPkg?: { name: string, bareSpecifier?: string }, targetPkg: { name: string, bareSpecifier?: string } }>,
  projectManifests: Array<{ importerId: string, manifest: ProjectManifest }> = []
): Set<string> {
  const applied = new Set<string>()

  const packageEntries = Object.entries(lockfile.packages ?? {}) as Array<[DepPath, PackageSnapshot]>
  const packageSnapshots = packageEntries.map(([, snapshot]) => snapshot)
  const importerSnapshots = Object.values(lockfile.importers)

  for (const override of parsedOverrides) {
    const targetName = override.targetPkg.name

    if (override.parentPkg != null) {
      const parentName = override.parentPkg.name
      const parentRange = override.parentPkg.bareSpecifier
      const parentRangeValid = parentRange == null || semver.validRange(parentRange) != null

      for (const { importerId, manifest: projectManifest } of projectManifests) {
        if (projectManifest.name !== parentName) continue
        if (parentRange != null) {
          const projectVersion = projectManifest.version
          if (projectVersion == null) continue
          if (!parentRangeValid || !semver.satisfies(projectVersion, parentRange)) continue
        }
        // Importer snapshots don't carry peerDependencies, so check the
        // manifest directly for that field. The other three groups are
        // covered by the importer entry (resolved deps share the same
        // names as the manifest).
        const importer = lockfile.importers[importerId as ProjectId]
        const matched =
          (importer != null && (
            depEntryMatches(importer.dependencies, targetName) ||
            depEntryMatches(importer.devDependencies, targetName) ||
            depEntryMatches(importer.optionalDependencies, targetName)
          )) ||
          depEntryMatches(projectManifest.peerDependencies, targetName)
        if (matched) {
          applied.add(override.selector)
          break
        }
      }
      if (applied.has(override.selector)) continue

      for (const [depPath, snapshot] of packageEntries) {
        const { name, version } = nameVerFromPkgSnapshot(depPath, snapshot)
        if (name !== parentName) continue
        if (parentRange != null && (version == null || !parentRangeValid || !semver.satisfies(version, parentRange))) continue
        if (
          depEntryMatches(snapshot.dependencies, targetName) ||
          depEntryMatches(snapshot.optionalDependencies, targetName) ||
          depEntryMatches(snapshot.peerDependencies, targetName)
        ) {
          applied.add(override.selector)
          break
        }
      }
    } else {
      for (const importer of importerSnapshots) {
        if (
          depEntryMatches(importer.dependencies, targetName) ||
          depEntryMatches(importer.devDependencies, targetName) ||
          depEntryMatches(importer.optionalDependencies, targetName)
        ) {
          applied.add(override.selector)
          break
        }
      }
      if (applied.has(override.selector)) continue
      for (const snapshot of packageSnapshots) {
        if (
          depEntryMatches(snapshot.dependencies, targetName) ||
          depEntryMatches(snapshot.optionalDependencies, targetName) ||
          depEntryMatches(snapshot.peerDependencies, targetName)
        ) {
          applied.add(override.selector)
          break
        }
      }
    }
  }

  return applied
}

function depEntryMatches (
  deps: Record<string, string> | undefined,
  targetName: string
): boolean {
  if (deps == null) return false
  return deps[targetName] != null
}
