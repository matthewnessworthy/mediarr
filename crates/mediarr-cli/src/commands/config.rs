//! Config command implementation.
//!
//! Supports viewing the full config, getting individual values by dotted path,
//! and setting values with validation. Enum fields and templates are validated
//! before writing.

use mediarr_core::{Config, MediaType, TemplateEngine};

use crate::ConfigArgs;

/// Execute the config command.
pub fn execute(args: ConfigArgs) -> anyhow::Result<()> {
    let config_path = mediarr_core::config::default_config_path()?;
    let config = Config::load(&config_path)?;

    if let Some(ref key) = args.get {
        // --get: navigate dotted path and print value
        let value = toml::Value::try_from(&config)?;
        let found = navigate_path(&value, key)?;
        print_value(found);
    } else if let Some(ref kv) = args.set {
        // --set key value
        let key = &kv[0];
        let val = &kv[1];

        // Validate enum fields before modifying
        validate_field(key, val)?;

        // Serialize current config to toml::Value, modify, deserialize back
        let mut value = toml::Value::try_from(&config)?;
        set_path(&mut value, key, val)?;

        // Round-trip through Config to verify validity
        let updated: Config = value.try_into()?;
        updated.save(&config_path)?;

        println!("Updated {key} = {val}");
    } else {
        // No flags: print full config as TOML
        let toml_str = toml::to_string_pretty(&config)?;
        print!("{toml_str}");
    }

    Ok(())
}

/// Navigate a dotted path (e.g. "general.output_dir") through a TOML value tree.
fn navigate_path<'a>(value: &'a toml::Value, path: &str) -> anyhow::Result<&'a toml::Value> {
    let segments: Vec<&str> = path.split('.').collect();
    let mut current = value;

    for segment in &segments {
        current = match current {
            toml::Value::Table(table) => table
                .get(*segment)
                .ok_or_else(|| anyhow::anyhow!("key not found: {path}"))?,
            toml::Value::Array(arr) => {
                let idx: usize = segment
                    .parse()
                    .map_err(|_| anyhow::anyhow!("expected array index, got: {segment}"))?;
                arr.get(idx)
                    .ok_or_else(|| anyhow::anyhow!("array index {idx} out of bounds"))?
            }
            _ => anyhow::bail!("cannot navigate into {segment}: not a table or array"),
        };
    }

    Ok(current)
}

/// Set a value at a dotted path in a TOML value tree.
fn set_path(root: &mut toml::Value, path: &str, val: &str) -> anyhow::Result<()> {
    let segments: Vec<&str> = path.split('.').collect();
    let mut current = root;

    // Navigate to parent
    for segment in &segments[..segments.len() - 1] {
        current = match current {
            toml::Value::Table(table) => table
                .get_mut(*segment)
                .ok_or_else(|| anyhow::anyhow!("key not found: {segment}"))?,
            toml::Value::Array(arr) => {
                let idx: usize = segment
                    .parse()
                    .map_err(|_| anyhow::anyhow!("expected array index, got: {segment}"))?;
                arr.get_mut(idx)
                    .ok_or_else(|| anyhow::anyhow!("array index {idx} out of bounds"))?
            }
            _ => anyhow::bail!("cannot navigate into {segment}: not a table or array"),
        };
    }

    // Set the leaf value
    let leaf_key = segments
        .last()
        .ok_or_else(|| anyhow::anyhow!("empty key path"))?;
    match current {
        toml::Value::Table(table) => {
            // Preserve the type of the existing value
            let new_value = if let Some(existing) = table.get(*leaf_key) {
                coerce_value(val, existing)?
            } else {
                // New key -- treat as string
                toml::Value::String(val.to_string())
            };
            table.insert(leaf_key.to_string(), new_value);
        }
        _ => anyhow::bail!("parent of {leaf_key} is not a table"),
    }

    Ok(())
}

/// Coerce a string value to match the type of an existing TOML value.
fn coerce_value(val: &str, existing: &toml::Value) -> anyhow::Result<toml::Value> {
    match existing {
        toml::Value::String(_) => Ok(toml::Value::String(val.to_string())),
        toml::Value::Integer(_) => {
            let n: i64 = val
                .parse()
                .map_err(|_| anyhow::anyhow!("expected integer, got: {val}"))?;
            Ok(toml::Value::Integer(n))
        }
        toml::Value::Float(_) => {
            let f: f64 = val
                .parse()
                .map_err(|_| anyhow::anyhow!("expected float, got: {val}"))?;
            Ok(toml::Value::Float(f))
        }
        toml::Value::Boolean(_) => {
            let b: bool = val
                .parse()
                .map_err(|_| anyhow::anyhow!("expected boolean, got: {val}"))?;
            Ok(toml::Value::Boolean(b))
        }
        _ => Ok(toml::Value::String(val.to_string())),
    }
}

/// Validate enum fields and template values before setting.
fn validate_field(key: &str, val: &str) -> anyhow::Result<()> {
    let leaf = key.rsplit('.').next().unwrap_or(key);

    match leaf {
        "operation" => {
            let allowed = ["move", "copy"];
            if !allowed.contains(&val.to_lowercase().as_str()) {
                anyhow::bail!("invalid operation: {val} (allowed: {})", allowed.join(", "));
            }
        }
        "conflict_strategy" => {
            let allowed = ["skip", "overwrite", "suffix"];
            if !allowed.contains(&val.to_lowercase().as_str()) {
                anyhow::bail!(
                    "invalid conflict_strategy: {val} (allowed: {})",
                    allowed.join(", ")
                );
            }
        }
        "non_preferred_action" => {
            let allowed = ["ignore", "backup", "keep_all", "review"];
            if !allowed.contains(&val.to_lowercase().as_str()) {
                anyhow::bail!(
                    "invalid non_preferred_action: {val} (allowed: {})",
                    allowed.join(", ")
                );
            }
        }
        "mode" if key.contains("watchers") => {
            let allowed = ["auto", "review"];
            if !allowed.contains(&val.to_lowercase().as_str()) {
                anyhow::bail!(
                    "invalid watcher mode: {val} (allowed: {})",
                    allowed.join(", ")
                );
            }
        }
        _ => {}
    }

    // Validate template fields
    if key.starts_with("templates.") {
        let media_type = match leaf {
            "movie" => Some(MediaType::Movie),
            "series" => Some(MediaType::Series),
            _ => None,
        };

        if let Some(mt) = media_type {
            let engine = TemplateEngine::new();
            let warnings = engine.validate(val, &mt);
            for w in &warnings {
                eprintln!("Warning: {} - {}", w.variable, w.message);
            }
        }
    }

    Ok(())
}

/// Print a TOML value to stdout.
fn print_value(value: &toml::Value) {
    match value {
        toml::Value::Table(_) | toml::Value::Array(_) => {
            let pretty = toml::to_string_pretty(value).unwrap_or_else(|_| format!("{value}"));
            print!("{pretty}");
        }
        toml::Value::String(s) => println!("{s}"),
        other => println!("{other}"),
    }
}
