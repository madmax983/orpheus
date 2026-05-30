//! The `loader` module handles the resolution and loading of Orpheus source files.
//!
//! This module implements the logic for reading `.ode` files from disk, recursively
//! resolving `import` statements, preventing cyclic dependencies during resolution,
//! and compiling the modules with strict Hindley-Milner type inference.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostics::LoadError;
use crate::eval::eval_into_bindings;
use crate::types::infer_into_bindings;
use crate::{ReplMode, Type, TypedModule, Value};

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImportSpec {
    path: Box<str>,
    names: Vec<String>,
}

/// The result of parsing, typechecking, and evaluating an Orpheus module.
///
/// Contains the fully inferred type bindings and fully evaluated runtime values
/// for all top-level statements. Also tracks the name of the final binding
/// so the REPL knows what pattern to make active automatically.
///
/// # Examples
///
/// ```
/// use orpheus_lang::StrictLoadedFile;
/// use std::collections::BTreeMap;
///
/// let file = StrictLoadedFile {
///     type_bindings: BTreeMap::new(),
///     value_bindings: BTreeMap::new(),
///     last_binding_name: None,
/// };
/// ```
#[derive(Clone, Debug)]
pub struct StrictLoadedFile {
    /// The map of fully inferred static types for all top-level bindings in the file.
    pub type_bindings: BTreeMap<String, Type>,
    /// The map of fully evaluated runtime values for all top-level bindings in the file.
    pub value_bindings: BTreeMap<String, Value>,
    /// The identifier name of the very last statement evaluated in the file, if any.
    ///
    /// This is typically used by the REPL to automatically select and activate the most
    /// recently defined pattern.
    pub last_binding_name: Option<String>,
}

/// Loads a strict `.ode` file, resolves its imports, and infers its bindings.
///
/// # Examples
///
/// ```no_run
/// use orpheus_lang::load_file_strict;
///
/// let typed_module = load_file_strict("main.ode").unwrap();
/// ```
///
/// # Errors
///
/// Returns [`LoadError`] when file I/O fails, an import directive is malformed,
/// an imported name is missing, or strict-mode inference fails.
pub fn load_file_strict(path: impl AsRef<Path>) -> Result<TypedModule, LoadError> {
    let mut visiting = BTreeSet::new();
    let loaded = load_file_strict_inner(path.as_ref(), &mut visiting)?;
    Ok(TypedModule::new(loaded.type_bindings))
}

/// Loads an Orpheus source file from disk and performs strict compilation
/// for runtime evaluation.
///
/// This resolves any `import` statements recursively while preventing
/// cyclic dependencies. It performs full type inference to ensure
/// type safety before generating the loaded module.
///
/// # Parameters
/// - `path`: The file path to the entry `.ode` source file.
///
/// # Errors
/// Returns a [`LoadError`] if the file cannot be read, if a parsing/type error
/// occurs, or if a cyclic dependency is detected.
///
/// # Examples
///
/// ```no_run
/// use orpheus_lang::load_file_runtime_strict;
///
/// // This will typecheck and load `main.ode` and all its dependencies.
/// let module = load_file_runtime_strict("main.ode").unwrap();
/// ```
pub fn load_file_runtime_strict(path: impl AsRef<Path>) -> Result<StrictLoadedFile, LoadError> {
    let mut visiting = BTreeSet::new();
    load_file_strict_inner(path.as_ref(), &mut visiting)
}

fn load_file_strict_inner(
    path: &Path,
    visiting: &mut BTreeSet<PathBuf>,
) -> Result<StrictLoadedFile, LoadError> {
    let canonical = path.canonicalize().map_err(|error| {
        LoadError::new(format!(
            "{}: {}",
            path.display(),
            match error.kind() {
                std::io::ErrorKind::NotFound => "file not found".to_string(),
                std::io::ErrorKind::PermissionDenied => "permission denied".to_string(),
                _ => error.to_string(),
            }
        ))
    })?;
    if !visiting.insert(canonical.clone()) {
        return Err(LoadError::new(format!(
            "{}: import cycle detected",
            canonical.display()
        )));
    }

    let source = fs::read_to_string(&canonical).map_err(|error| {
        LoadError::new(format!(
            "{}: {}",
            canonical.display(),
            match error.kind() {
                std::io::ErrorKind::NotFound => "file not found".to_string(),
                std::io::ErrorKind::PermissionDenied => "permission denied".to_string(),
                _ => error.to_string(),
            }
        ))
    })?;
    let parent = canonical.parent().ok_or_else(|| {
        LoadError::new(format!(
            "{}: cannot resolve the parent directory for imports",
            canonical.display()
        ))
    })?;
    let (imports, body) = split_imports(&source, &canonical)?;
    let mut type_bindings = BTreeMap::<String, Type>::new();
    let mut value_bindings = BTreeMap::<String, Value>::new();

    for import in imports {
        let imported_path = parent.join(import.path.as_ref());
        let mut imported_module = load_file_strict_inner(&imported_path, visiting)?;
        for name in import.names {
            let Some(ty) = imported_module.type_bindings.remove(&name) else {
                return Err(LoadError::new(format!(
                    "{}: unresolved name `{name}` imported from {}",
                    canonical.display(),
                    imported_path.display()
                )));
            };
            let Some(value) = imported_module.value_bindings.remove(&name) else {
                return Err(LoadError::new(format!(
                    "{}: unresolved value `{name}` imported from {}",
                    canonical.display(),
                    imported_path.display()
                )));
            };
            type_bindings.insert(name.clone(), ty);
            value_bindings.insert(name, value);
        }
    }

    let result = if body.trim().is_empty() {
        Ok(StrictLoadedFile {
            type_bindings,
            value_bindings,
            last_binding_name: None,
        })
    } else {
        let last_type_binding = infer_into_bindings(&body, ReplMode::Strict, &mut type_bindings)
            .map_err(|error| LoadError::new(format!("{}: {error}", canonical.display())))?;
        let last_value_binding =
            eval_into_bindings(&body, ReplMode::Strict, &mut value_bindings)
                .map_err(|error| LoadError::new(format!("{}: {error}", canonical.display())))?;
        debug_assert_eq!(
            last_type_binding.as_ref().map(|(name, _)| name),
            last_value_binding.as_ref().map(|(name, _)| name)
        );

        Ok(StrictLoadedFile {
            type_bindings,
            value_bindings,
            last_binding_name: last_type_binding.map(|(name, _)| name),
        })
    };

    visiting.remove(&canonical);
    result
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
