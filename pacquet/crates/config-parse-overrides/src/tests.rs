use crate::{
    ConversionResult, PackageSelector, ParseOverridesError, SkippedResolution, VersionOverride,
    convert_resolutions_to_overrides, create_overrides_map_from_parsed, parse_overrides,
    parse_pkg_and_parent_selector,
};
use pacquet_catalogs_types::{Catalog, Catalogs};
use std::collections::HashMap;

fn vo(
    selector: &str,
    new_bare: &str,
    parent: Option<PackageSelector>,
    target: PackageSelector,
) -> VersionOverride {
    VersionOverride {
        selector: selector.to_string(),
        parent_pkg: parent,
        target_pkg: target,
        new_bare_specifier: new_bare.to_string(),
    }
}

fn sel(name: &str, bare: Option<&str>) -> PackageSelector {
    PackageSelector { name: name.to_string(), bare_specifier: bare.map(str::to_owned) }
}

/// `HashMap` iteration order is unspecified, so when comparing
/// multi-entry outputs we sort by `selector` on both sides.
fn sorted(mut overrides: Vec<VersionOverride>) -> Vec<VersionOverride> {
    overrides.sort_by(|lhs, rhs| lhs.selector.cmp(&rhs.selector));
    overrides
}

#[test]
fn parses_bare_name_override() {
    let input = HashMap::from([("foo".to_string(), "1".to_string())]);
    let out = parse_overrides(&input, &Catalogs::new()).unwrap();
    assert_eq!(out, vec![vo("foo", "1", None, sel("foo", None))]);
}

#[test]
fn parses_name_at_version_override() {
    let input = HashMap::from([("foo@2".to_string(), "1".to_string())]);
    let out = parse_overrides(&input, &Catalogs::new()).unwrap();
    assert_eq!(out, vec![vo("foo@2", "1", None, sel("foo", Some("2")))]);
}

#[test]
fn parses_range_operators_in_target() {
    let input = HashMap::from([
        ("foo@>2".to_string(), "1".to_string()),
        ("foo@3 || >=2".to_string(), "1".to_string()),
    ]);
    let out = sorted(parse_overrides(&input, &Catalogs::new()).unwrap());
    assert_eq!(
        out,
        sorted(vec![
            vo("foo@>2", "1", None, sel("foo", Some(">2"))),
            vo("foo@3 || >=2", "1", None, sel("foo", Some("3 || >=2"))),
        ]),
    );
}

#[test]
fn parses_parent_child_selectors() {
    let input = HashMap::from([
        ("bar>foo".to_string(), "2".to_string()),
        ("bar@1>foo".to_string(), "2".to_string()),
        ("bar>foo@1".to_string(), "2".to_string()),
        ("bar@1>foo@1".to_string(), "2".to_string()),
    ]);
    let out = sorted(parse_overrides(&input, &Catalogs::new()).unwrap());
    assert_eq!(
        out,
        sorted(vec![
            vo("bar>foo", "2", Some(sel("bar", None)), sel("foo", None)),
            vo("bar@1>foo", "2", Some(sel("bar", Some("1"))), sel("foo", None)),
            vo("bar>foo@1", "2", Some(sel("bar", None)), sel("foo", Some("1"))),
            vo("bar@1>foo@1", "2", Some(sel("bar", Some("1"))), sel("foo", Some("1"))),
        ]),
    );
}

#[test]
fn range_operator_on_parent_does_not_split() {
    // Without the `[^ |@]>` constraint, `foo@>2>bar@>2` would split
    // at the first `>` (inside the `>2` range). Mirrors upstream's
    // exact disambiguation.
    let input = HashMap::from([
        ("foo@>2>bar@>2".to_string(), "1".to_string()),
        ("foo@3 || >=2>bar@3 || >=2".to_string(), "1".to_string()),
    ]);
    let out = sorted(parse_overrides(&input, &Catalogs::new()).unwrap());
    assert_eq!(
        out,
        sorted(vec![
            vo("foo@>2>bar@>2", "1", Some(sel("foo", Some(">2"))), sel("bar", Some(">2"))),
            vo(
                "foo@3 || >=2>bar@3 || >=2",
                "1",
                Some(sel("foo", Some("3 || >=2"))),
                sel("bar", Some("3 || >=2")),
            ),
        ]),
    );
}

#[test]
fn rejects_invalid_selector() {
    let input = HashMap::from([("%".to_string(), "2".to_string())]);
    assert_eq!(
        parse_overrides(&input, &Catalogs::new()).unwrap_err(),
        ParseOverridesError::InvalidSelector { selector: "%".to_string() },
    );
}

#[test]
fn rejects_invalid_selector_with_whitespace() {
    // `foo > bar` — the regex requires the byte before `>` to be
    // non-space, so the parser sees no parent>child split and falls
    // through to `parse_pkg_selector("foo > bar")`, which fails
    // because `parse_wanted_dependency` doesn't validate the alias.
    let input = HashMap::from([("foo > bar".to_string(), "2".to_string())]);
    assert_eq!(
        parse_overrides(&input, &Catalogs::new()).unwrap_err(),
        ParseOverridesError::InvalidSelector { selector: "foo > bar".to_string() },
    );
}

#[test]
fn parse_pkg_and_parent_selector_lone_target() {
    assert_eq!(parse_pkg_and_parent_selector("foo").unwrap(), (None, sel("foo", None)));
}

#[test]
fn parse_pkg_and_parent_selector_parent_child() {
    assert_eq!(
        parse_pkg_and_parent_selector("bar@1>foo@2").unwrap(),
        (Some(sel("bar", Some("1"))), sel("foo", Some("2"))),
    );
}

#[test]
fn catalog_protocol_with_missing_entry_errors() {
    // An empty catalog table can never resolve a `catalog:` value;
    // upstream surfaces this as `ERR_PNPM_CATALOG_IN_OVERRIDES` with
    // the underlying "No catalog entry" message.
    let input = HashMap::from([("foo".to_string(), "catalog:default".to_string())]);
    let err = parse_overrides(&input, &Catalogs::new()).unwrap_err();
    let ParseOverridesError::CatalogInOverrides { message } = err else {
        panic!("expected CatalogInOverrides, got {err:?}");
    };
    assert!(
        message.contains("foo") && message.contains("default"),
        "message should mention target and catalog name, got: {message}",
    );
}

/// `catalog:` resolves to the catalog's specifier when the entry exists.
/// Matches upstream's
/// [`parseOverrides`](https://github.com/pnpm/pnpm/blob/4a36b9a110/config/parse-overrides/src/index.ts#L28-L41)
/// behavior where `matchCatalogResolveResult.found` returns the
/// resolved specifier and the entry's `newBareSpecifier` is rewritten
/// to it.
#[test]
fn catalog_protocol_resolves_to_catalog_specifier() {
    let mut catalogs = Catalogs::new();
    let mut default = Catalog::new();
    default.insert("foo".to_string(), "^1.2.3".to_string());
    catalogs.insert("default".to_string(), default);

    let input = HashMap::from([("foo".to_string(), "catalog:".to_string())]);
    let out = parse_overrides(&input, &catalogs).unwrap();
    assert_eq!(out, vec![vo("foo", "^1.2.3", None, sel("foo", None))]);
}

/// `catalog:name` looks up the named catalog by name.
#[test]
fn catalog_protocol_with_named_catalog_resolves() {
    let mut catalogs = Catalogs::new();
    let mut shared = Catalog::new();
    shared.insert("bar".to_string(), "2.0.0".to_string());
    catalogs.insert("shared".to_string(), shared);

    let input = HashMap::from([("bar".to_string(), "catalog:shared".to_string())]);
    let out = parse_overrides(&input, &catalogs).unwrap();
    assert_eq!(out, vec![vo("bar", "2.0.0", None, sel("bar", None))]);
}

/// `create_overrides_map_from_parsed` flattens the parsed entries back
/// into the `selector → newBareSpecifier` map shape — with catalog
/// resolution already applied. Mirrors upstream's
/// [`createOverridesMapFromParsed`](https://github.com/pnpm/pnpm/blob/4a36b9a110/lockfile/settings-checker/src/createOverridesMapFromParsed.ts).
#[test]
fn create_overrides_map_returns_resolved_specifiers() {
    let mut catalogs = Catalogs::new();
    let mut default = Catalog::new();
    default.insert("foo".to_string(), "^1.2.3".to_string());
    catalogs.insert("default".to_string(), default);

    let input = HashMap::from([
        ("foo".to_string(), "catalog:".to_string()),
        ("bar".to_string(), "2.0.0".to_string()),
    ]);
    let parsed = parse_overrides(&input, &catalogs).unwrap();
    let map = create_overrides_map_from_parsed(&parsed);
    assert_eq!(map.get("foo").map(String::as_str), Some("^1.2.3"));
    assert_eq!(map.get("bar").map(String::as_str), Some("2.0.0"));
}

fn cr(overrides: Vec<(&str, &str)>, skipped: Vec<(&str, &str)>) -> ConversionResult {
    ConversionResult {
        overrides: overrides.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        skipped: skipped
            .into_iter()
            .map(|(sel, reason)| SkippedResolution {
                selector: sel.to_string(),
                reason: reason.to_string(),
            })
            .collect(),
    }
}

#[test]
fn resolutions_global_override_passthrough() {
    let input =
        vec![("foo".to_string(), "1.0.0".to_string()), ("bar".to_string(), "2.0.0".to_string())];
    let result = convert_resolutions_to_overrides(&input);
    assert_eq!(result, cr(vec![("foo", "1.0.0"), ("bar", "2.0.0")], vec![]));
}

#[test]
fn resolutions_parent_child_slash_to_gt() {
    let input = vec![("parent/child".to_string(), "1.0.0".to_string())];
    let result = convert_resolutions_to_overrides(&input);
    assert_eq!(result, cr(vec![("parent>child", "1.0.0")], vec![]));
}

#[test]
fn resolutions_scoped_global_override() {
    let input = vec![("@babel/core".to_string(), "7.0.0".to_string())];
    let result = convert_resolutions_to_overrides(&input);
    assert_eq!(result, cr(vec![("@babel/core", "7.0.0")], vec![]));
}

#[test]
fn resolutions_scoped_parent_with_child() {
    let input = vec![("@scope/pkg/child".to_string(), "1.0.0".to_string())];
    let result = convert_resolutions_to_overrides(&input);
    assert_eq!(result, cr(vec![("@scope/pkg>child", "1.0.0")], vec![]));
}

#[test]
fn resolutions_non_scoped_parent_with_scoped_child() {
    let input = vec![("parent/@scope/child".to_string(), "1.0.0".to_string())];
    let result = convert_resolutions_to_overrides(&input);
    assert_eq!(result, cr(vec![("parent>@scope/child", "1.0.0")], vec![]));
}

#[test]
fn resolutions_skips_glob_patterns() {
    let input = vec![("**/foo".to_string(), "1.0.0".to_string())];
    let result = convert_resolutions_to_overrides(&input);
    assert_eq!(result.overrides, vec![]);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].selector, "**/foo");
}

#[test]
fn resolutions_skips_berry_qualifiers() {
    let input = vec![("pkg@npm:1.0.0".to_string(), "2.0.0".to_string())];
    let result = convert_resolutions_to_overrides(&input);
    assert_eq!(result.overrides, vec![]);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].selector, "pkg@npm:1.0.0");
}

#[test]
fn resolutions_mixed_entries() {
    let input = vec![
        ("foo".to_string(), "1.0.0".to_string()),
        ("parent/child".to_string(), "2.0.0".to_string()),
        ("@scope/pkg/nested".to_string(), "3.0.0".to_string()),
        ("**/globby".to_string(), "4.0.0".to_string()),
        ("bar@npm:1".to_string(), "5.0.0".to_string()),
    ];
    let result = convert_resolutions_to_overrides(&input);
    assert_eq!(result.overrides.len(), 3);
    assert_eq!(result.skipped.len(), 2);
}

#[test]
fn resolutions_empty_input() {
    let result = convert_resolutions_to_overrides(&[]);
    assert_eq!(result, cr(vec![], vec![]));
}

#[test]
fn resolutions_deep_nesting_first_slash_only() {
    let input = vec![("a/b/c".to_string(), "1.0.0".to_string())];
    let result = convert_resolutions_to_overrides(&input);
    assert_eq!(result, cr(vec![("a>b/c", "1.0.0")], vec![]));
}
