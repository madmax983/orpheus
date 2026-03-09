use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostics::LoadError;
use crate::types::infer_into_bindings;
use crate::{ReplMode, Type, TypedModule};

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImportSpec {
    path: Box<str>,
    names: Vec<String>,
}

/// Loads a strict `.ode` file, resolves its imports, and infers its bindings.
///
/// # Errors
///
/// Returns [`LoadError`] when file I/O fails, an import directive is malformed,
/// an imported name is missing, or strict-mode inference fails.
pub fn load_file_strict(path: impl AsRef<Path>) -> Result<TypedModule, LoadError> {
    let mut visiting = BTreeSet::new();
    load_file_strict_inner(path.as_ref(), &mut visiting)
}

fn load_file_strict_inner(
    path: &Path,
    visiting: &mut BTreeSet<PathBuf>,
) -> Result<TypedModule, LoadError> {
    let canonical = path
        .canonicalize()
        .map_err(|error| LoadError::new(format!("{}: {error}", path.display())))?;
    if !visiting.insert(canonical.clone()) {
        return Err(LoadError::new(format!(
            "{}: import cycle detected",
            canonical.display()
        )));
    }

    let source = fs::read_to_string(&canonical)
        .map_err(|error| LoadError::new(format!("{}: {error}", canonical.display())))?;
    let parent = canonical.parent().ok_or_else(|| {
        LoadError::new(format!(
            "{}: cannot resolve the parent directory for imports",
            canonical.display()
        ))
    })?;
    let (imports, body) = split_imports(&source, &canonical)?;
    let mut bindings = BTreeMap::<String, Type>::new();

    for import in imports {
        let imported_path = parent.join(import.path.as_ref());
        let imported_module = load_file_strict_inner(&imported_path, visiting)?;
        for name in import.names {
            let Some(ty) = imported_module.get(&name).cloned() else {
                return Err(LoadError::new(format!(
                    "{}: unresolved name `{name}` imported from {}",
                    canonical.display(),
                    imported_path.display()
                )));
            };
            bindings.insert(name, ty);
        }
    }

    if !body.trim().is_empty() {
        infer_into_bindings(&body, ReplMode::Strict, &mut bindings)
            .map_err(|error| LoadError::new(format!("{}: {error}", canonical.display())))?;
    }
    let infer_result = TypedModule::new(bindings);

    visiting.remove(&canonical);
    Ok(infer_result)
}

fn split_imports(source: &str, path: &Path) -> Result<(Vec<ImportSpec>, String), LoadError> {
    let mut imports = Vec::new();
    let mut body_lines = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(import) = parse_import_line(trimmed, path)? {
            imports.push(import);
        } else {
            body_lines.push(trimmed.to_owned());
        }
    }

    Ok((imports, body_lines.join("\n")))
}

fn parse_import_line(line: &str, path: &Path) -> Result<Option<ImportSpec>, LoadError> {
    let Some(rest) = line.strip_prefix("use ") else {
        return Ok(None);
    };
    let Some(rest) = rest.strip_prefix('"') else {
        return Err(LoadError::new(format!(
            "{}: import path must start with a quoted filename",
            path.display()
        )));
    };
    let Some((import_path, trailing)) = rest.split_once('"') else {
        return Err(LoadError::new(format!(
            "{}: import path is missing a closing quote",
            path.display()
        )));
    };
    let trailing = trailing.trim();
    let Some(names) = trailing
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    else {
        return Err(LoadError::new(format!(
            "{}: import list must use parentheses",
            path.display()
        )));
    };

    let parsed_names = names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if parsed_names.is_empty() {
        return Err(LoadError::new(format!(
            "{}: import list must name at least one binding",
            path.display()
        )));
    }

    Ok(Some(ImportSpec {
        path: import_path.into(),
        names: parsed_names,
    }))
}
