use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

use indexmap::IndexMap;
use pacquet_config_parse_overrides::{ConversionResult, convert_resolutions_to_overrides};
use pacquet_package_manifest::PackageManifest;

pub struct MigrationOutcome {
    pub converted: IndexMap<String, String>,
}

pub fn maybe_migrate_resolutions(
    manifest: &PackageManifest,
    config_overrides: Option<&IndexMap<String, String>>,
    workspace_yaml_path: &Path,
) -> Option<MigrationOutcome> {
    let value = manifest.value();
    let resolutions = value.get("resolutions")?.as_object()?;
    if resolutions.is_empty() {
        return None;
    }
    if let Some(existing) = config_overrides
        && !existing.is_empty()
    {
        return None;
    }

    let entries: Vec<(String, String)> = resolutions
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect();

    let ConversionResult { overrides, skipped } = convert_resolutions_to_overrides(&entries);

    for skip in &skipped {
        eprintln!("warn: Cannot auto-migrate resolution \"{}\": {}", skip.selector, skip.reason);
    }

    if overrides.is_empty() {
        return None;
    }

    let is_ci = std::env::var("CI").is_ok();
    if is_ci || !io::stdin().is_terminal() {
        eprintln!(
            "info: {} resolution(s) in package.json could be migrated to overrides \
             in pnpm-workspace.yaml. Re-run in an interactive terminal to proceed.",
            overrides.len(),
        );
        return None;
    }

    eprint!(
        "Migrate {} resolution(s) from package.json to overrides in pnpm-workspace.yaml? [Y/n] ",
        overrides.len(),
    );
    let _ = io::stdout().flush();

    let mut answer = String::new();
    if io::stdin().lock().read_line(&mut answer).is_err() {
        return None;
    }
    let trimmed = answer.trim().to_lowercase();
    if !trimmed.is_empty() && trimmed != "y" && trimmed != "yes" {
        return None;
    }

    let converted: IndexMap<String, String> = overrides.into_iter().collect();

    if let Err(err) = write_overrides_to_yaml(workspace_yaml_path, &converted) {
        eprintln!("warn: Failed to write overrides to pnpm-workspace.yaml: {err}");
        return None;
    }

    if let Err(err) = remove_resolutions_from_manifest(manifest) {
        eprintln!("warn: Failed to remove resolutions from package.json: {err}");
        return None;
    }

    eprintln!(
        "info: Migrated {} resolution(s) to overrides in pnpm-workspace.yaml",
        converted.len(),
    );

    Some(MigrationOutcome { converted })
}

fn write_overrides_to_yaml(
    yaml_path: &Path,
    overrides: &IndexMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let existing_text = std::fs::read_to_string(yaml_path).unwrap_or_default();
    let mut lines: Vec<String> = existing_text.lines().map(str::to_string).collect();

    let indent = find_indent(&lines);
    lines.push(String::new());
    lines.push("overrides:".to_string());
    for (k, v) in overrides {
        lines.push(format!("{}{}: {}", indent, k, yaml_quote_value(v)));
    }

    let content = lines.join("\n") + "\n";
    std::fs::write(yaml_path, content)?;
    Ok(())
}

fn yaml_quote_value(value: &str) -> String {
    let needs_quoting = value.contains('\'')
        || value.contains('"')
        || value.contains(':')
        || value.contains('#')
        || value.contains('{')
        || value.contains('}')
        || value.contains('[')
        || value.contains(']')
        || value.contains(',')
        || value.contains('&')
        || value.contains('*')
        || value.contains('!')
        || value.contains('|')
        || value.contains('>')
        || value.contains('%')
        || value.contains('@')
        || value.contains('`')
        || value.starts_with(' ')
        || value.ends_with(' ')
        || value.starts_with('-')
        || value.starts_with('?');
    if needs_quoting {
        let escaped = value.replace('\'', "''");
        format!("'{}'", escaped)
    } else {
        value.to_string()
    }
}

fn find_indent(lines: &[String]) -> String {
    for line in lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with('-') || trimmed.contains(':') {
            let leading = line.len() - trimmed.len();
            if leading > 0 {
                return line[..leading].to_string();
            }
        }
    }
    "  ".to_string()
}

fn remove_resolutions_from_manifest(
    manifest: &PackageManifest,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cloned = manifest.clone();
    if let Some(obj) = cloned.value_mut().as_object_mut() {
        obj.remove("resolutions");
    }
    cloned.save()?;
    Ok(())
}
