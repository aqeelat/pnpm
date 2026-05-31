import { describe, expect, test } from '@jest/globals'
import { convertResolutionsToOverrides } from '@pnpm/config.parse-overrides'

describe('convertResolutionsToOverrides', () => {
  test('passes through global overrides unchanged', () => {
    const result = convertResolutionsToOverrides({
      foo: '1.0.0',
      bar: '2.0.0',
    })
    expect(result.overrides).toEqual({ foo: '1.0.0', bar: '2.0.0' })
    expect(result.skipped).toEqual([])
  })

  test('converts parent/child selectors', () => {
    const result = convertResolutionsToOverrides({
      'parent/child': '1.0.0',
    })
    expect(result.overrides).toEqual({ 'parent>child': '1.0.0' })
    expect(result.skipped).toEqual([])
  })

  test('preserves scoped package as global override', () => {
    const result = convertResolutionsToOverrides({
      '@babel/core': '7.0.0',
    })
    expect(result.overrides).toEqual({ '@babel/core': '7.0.0' })
    expect(result.skipped).toEqual([])
  })

  test('converts scoped parent with child', () => {
    const result = convertResolutionsToOverrides({
      '@scope/pkg/child': '1.0.0',
    })
    expect(result.overrides).toEqual({ '@scope/pkg>child': '1.0.0' })
    expect(result.skipped).toEqual([])
  })

  test('converts non-scoped parent with scoped child', () => {
    const result = convertResolutionsToOverrides({
      'parent/@scope/child': '1.0.0',
    })
    expect(result.overrides).toEqual({ 'parent>@scope/child': '1.0.0' })
    expect(result.skipped).toEqual([])
  })

  test('skips glob patterns', () => {
    const result = convertResolutionsToOverrides({
      '**/foo': '1.0.0',
    })
    expect(result.overrides).toEqual({})
    expect(result.skipped).toEqual([
      { selector: '**/foo', reason: 'Yarn glob patterns (**) have no pnpm equivalent' },
    ])
  })

  test('skips Yarn Berry qualifiers', () => {
    const result = convertResolutionsToOverrides({
      'pkg@npm:1.0.0': '2.0.0',
    })
    expect(result.overrides).toEqual({})
    expect(result.skipped).toEqual([
      { selector: 'pkg@npm:1.0.0', reason: 'Yarn Berry package qualifiers (@pkg@npm:version) are not supported' },
    ])
  })

  test('handles mixed convertible and non-convertible entries', () => {
    const result = convertResolutionsToOverrides({
      foo: '1.0.0',
      'parent/child': '2.0.0',
      '@scope/pkg/nested': '3.0.0',
      '**/globby': '4.0.0',
      'bar@npm:1': '5.0.0',
    })
    expect(result.overrides).toEqual({
      foo: '1.0.0',
      'parent>child': '2.0.0',
      '@scope/pkg>nested': '3.0.0',
    })
    expect(result.skipped).toHaveLength(2)
    expect(result.skipped[0].selector).toBe('**/globby')
    expect(result.skipped[1].selector).toBe('bar@npm:1')
  })

  test('returns empty result for empty input', () => {
    const result = convertResolutionsToOverrides({})
    expect(result.overrides).toEqual({})
    expect(result.skipped).toEqual([])
  })

  test('converts deep nesting (first slash only)', () => {
    const result = convertResolutionsToOverrides({
      'a/b/c': '1.0.0',
    })
    expect(result.overrides).toEqual({ 'a>b/c': '1.0.0' })
  })
})
