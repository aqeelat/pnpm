import { matchCatalogResolveResult, resolveFromCatalog } from '@pnpm/catalogs.resolver'
import type { Catalogs } from '@pnpm/catalogs.types'
import { PnpmError } from '@pnpm/error'
import { parseWantedDependency } from '@pnpm/resolving.parse-wanted-dependency'

const DELIMITER_REGEX = /[^ |@]>/

export interface VersionOverride {
  selector: string
  parentPkg?: PackageSelector
  targetPkg: PackageSelector
  newBareSpecifier: string
}

export interface PackageSelector {
  name: string
  bareSpecifier?: string
}

export function parseOverrides (
  overrides: Record<string, string>,
  catalogs?: Catalogs
): VersionOverride[] {
  const _resolveFromCatalog = resolveFromCatalog.bind(null, catalogs ?? {})
  return Object.entries(overrides)
    .map(([selector, newBareSpecifier]) => {
      const result = parsePkgAndParentSelector(selector)
      const resolvedCatalog = matchCatalogResolveResult(_resolveFromCatalog({
        bareSpecifier: newBareSpecifier,
        alias: result.targetPkg.name,
      }), {
        found: ({ resolution }) => resolution.specifier,
        unused: () => undefined,
        misconfiguration: ({ error }) => {
          throw new PnpmError('CATALOG_IN_OVERRIDES', `Could not resolve a catalog in the overrides: ${error.message}`)
        },
      })
      return {
        selector,
        newBareSpecifier: resolvedCatalog ?? newBareSpecifier,
        ...result,
      }
    })
}

export function parsePkgAndParentSelector (selector: string): Pick<VersionOverride, 'parentPkg' | 'targetPkg'> {
  let delimiterIndex = selector.search(DELIMITER_REGEX)
  if (delimiterIndex !== -1) {
    delimiterIndex++
    const parentSelector = selector.substring(0, delimiterIndex)
    const childSelector = selector.substring(delimiterIndex + 1)
    return {
      parentPkg: parsePkgSelector(parentSelector),
      targetPkg: parsePkgSelector(childSelector),
    }
  }
  return {
    targetPkg: parsePkgSelector(selector),
  }
}

export interface SkippedResolution {
  selector: string
  reason: string
}

export interface ConversionResult {
  overrides: Record<string, string>
  skipped: SkippedResolution[]
}

export function convertResolutionsToOverrides (
  resolutions: Record<string, string>
): ConversionResult {
  const overrides: Record<string, string> = {}
  const skipped: SkippedResolution[] = []

  for (const [selector, spec] of Object.entries(resolutions)) {
    if (selector.includes('**')) {
      skipped.push({ selector, reason: 'Yarn glob patterns (**) have no pnpm equivalent' })
      continue
    }
    if (selector.includes('@npm:') || /@[\w-]+@/.test(selector)) {
      skipped.push({ selector, reason: 'Yarn Berry package qualifiers (@pkg@npm:version) are not supported' })
      continue
    }

    overrides[convertSelector(selector)] = spec
  }

  return { overrides, skipped }
}

function convertSelector (selector: string): string {
  const match = selector.match(/^(@[^/]+\/[^/]+)\/(.+)$/)
  if (match) {
    return `${match[1]}>${match[2]}`
  }

  const slashIndex = selector.indexOf('/')
  if (slashIndex !== -1 && selector[0] !== '@') {
    return selector.substring(0, slashIndex) + '>' + selector.substring(slashIndex + 1)
  }

  return selector
}

function parsePkgSelector (selector: string): PackageSelector {
  const wantedDep = parseWantedDependency(selector)
  if (!wantedDep.alias) {
    throw new PnpmError('INVALID_SELECTOR', `Cannot parse the "${selector}" selector`)
  }
  return {
    name: wantedDep.alias,
    bareSpecifier: wantedDep.bareSpecifier,
  }
}
