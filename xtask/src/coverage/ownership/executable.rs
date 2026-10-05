//! The executable test owner adapter: a (package, binary, module path,
//! function) selector resolved against the actual compiled module tree.
//!
//! The adapter supports exactly two workspace packages and Cargo's default
//! target discovery: the package's `src/lib.rs` library unit tests (`lib`) and
//! its auto-discovered `tests/<binary>.rs` or `tests/<binary>/main.rs` test
//! binaries. From the crate root it follows the `mod` chain named by the
//! module path, applying Rust's module file rules (`name.rs`/`name/mod.rs`,
//! `#[path]` relative to the declaring file, inline `mod name { .. }` blocks),
//! and requires the selected function to be an unconditional `#[test]` in the
//! final module. Anything it cannot resolve syntactically fails closed.
use syn::ext::IdentExt;

pub(super) const RENDERER: &str = "excalidraw-render";
pub(super) const COMPARE: &str = "excalidraw-render-compare";
/// The workspace packages whose test targets the adapter can resolve.
const PACKAGES: [&str; 2] = [RENDERER, COMPARE];

pub(super) fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// A module path from the crate root: empty for the root, else `a::b::c`.
pub(super) fn module_segments(module: &str) -> Result<Vec<&str>, String> {
    if module.is_empty() {
        return Ok(vec![]);
    }
    let segments: Vec<_> = module.split("::").collect();
    if segments.iter().any(|s| !valid_name(s)) {
        return Err("invalid test owner module path".into());
    }
    Ok(segments)
}

/// The exact libtest invocation selecting one test of one compiled target.
pub(super) fn command(package: &str, binary: &str, module: &str, function: &str) -> Vec<String> {
    let mut args: Vec<String> = ["cargo", "test", "--locked", "--release", "-p", package]
        .into_iter()
        .map(String::from)
        .collect();
    if binary == "lib" {
        args.push("--lib".into());
    } else {
        args.extend(["--test".into(), binary.into()]);
    }
    args.push(if module.is_empty() {
        function.into()
    } else {
        format!("{module}::{function}")
    });
    args.extend(["--".into(), "--exact".into()]);
    args
}

fn manifest(path: &str, read: &dyn Fn(&str) -> Option<String>) -> Result<String, String> {
    read(path).ok_or_else(|| format!("missing test target manifest {path}"))
}

/// The package uses Cargo's default target layout for the selected target and
/// is an explicit, unexcluded workspace member.
pub(super) fn package_layout(
    name: &str,
    package: &str,
    workspace: &str,
    library: bool,
) -> Result<(), String> {
    let package: toml::Value =
        toml::from_str(package).map_err(|e| format!("invalid package manifest: {e}"))?;
    let workspace: toml::Value =
        toml::from_str(workspace).map_err(|e| format!("invalid workspace manifest: {e}"))?;
    let section = package.get("package");
    let flag = |key: &str| {
        section
            .and_then(|p| p.get(key))
            .and_then(toml::Value::as_bool)
    };
    if section
        .and_then(|p| p.get("name"))
        .and_then(toml::Value::as_str)
        != Some(name)
    {
        return Err("unsupported test target package".into());
    }
    if library {
        let lib = package.get("lib");
        if flag("autolib") == Some(false)
            || lib.is_some_and(|l| l.as_table().is_none())
            || lib
                .and_then(|l| l.get("path"))
                .is_some_and(|p| p.as_str() != Some("src/lib.rs"))
            || ["test", "harness"].iter().any(|key| {
                lib.and_then(|l| l.get(key))
                    .is_some_and(|v| v.as_bool() != Some(true))
            })
            || lib
                .and_then(|l| l.get("proc-macro"))
                .is_some_and(|v| v.as_bool() != Some(false))
        {
            return Err("unsupported library test target layout".into());
        }
    } else if flag("autotests") == Some(false) || package.get("test").is_some() {
        // Explicit `[[test]]` targets may rename, redirect or disable the
        // harness of a binary; only auto-discovered harness binaries qualify.
        return Err("unsupported test binary target layout".into());
    }
    let workspace = workspace
        .get("workspace")
        .ok_or("missing workspace target layout")?;
    let member = format!("crates/{name}");
    if workspace.get("exclude").is_some()
        || !workspace
            .get("members")
            .and_then(toml::Value::as_array)
            .is_some_and(|members| members.iter().any(|m| m.as_str() == Some(&member)))
    {
        return Err("test target requires explicit, unexcluded workspace membership".into());
    }
    Ok(())
}

/// The crate root file of the selected target, after checking its layout.
pub(super) fn target_root(
    package: &str,
    binary: &str,
    read: &dyn Fn(&str) -> Option<String>,
) -> Result<String, String> {
    if !PACKAGES.contains(&package) || !valid_name(binary) {
        return Err("unsupported executable owner path/binary".into());
    }
    let library = binary == "lib";
    package_layout(
        package,
        &manifest(&format!("crates/{package}/Cargo.toml"), read)?,
        &manifest("Cargo.toml", read)?,
        library,
    )?;
    if library {
        return Ok(format!("crates/{package}/src/lib.rs"));
    }
    let candidates = [
        format!("crates/{package}/tests/{binary}.rs"),
        format!("crates/{package}/tests/{binary}/main.rs"),
    ];
    let present: Vec<_> = candidates
        .into_iter()
        .filter(|path| read(path).is_some())
        .collect();
    match <[String; 1]>::try_from(present) {
        Ok([root]) => Ok(root),
        Err(_) => Err("test binary is not an auto-discovered target".into()),
    }
}

/// One module scope on the resolved chain.
struct Scope {
    items: Vec<syn::Item>,
    /// The file holding this scope.
    file: String,
    /// The directory in which this scope's `mod name;` children are found.
    dir: String,
    inline: bool,
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Join a relative module path onto a repository directory, collapsing `.`
/// and `..` without leaving the repository.
fn join(dir: &str, relative: &str) -> Result<String, String> {
    if relative.starts_with('/') || relative.contains('\\') {
        return Err("unsupported module path attribute".into());
    }
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop().ok_or("module path escapes the repository")?;
            }
            part => parts.push(part),
        }
    }
    Ok(parts.join("/"))
}

fn parse_file(path: &str, text: &str) -> Result<syn::File, String> {
    let file = syn::parse_file(text).map_err(|e| format!("invalid test source {path}: {e}"))?;
    // A crate- or module-level inner attribute may disable or reconfigure
    // every test below it; only documentation is admitted.
    if file.attrs.iter().any(|attr| !attr.path().is_ident("doc")) {
        return Err("test owner source contains unsupported attributes".into());
    }
    Ok(file)
}

fn cfg_test(attr: &syn::Attribute) -> bool {
    matches!(attr.style, syn::AttrStyle::Outer)
        && attr.path().is_ident("cfg")
        && attr.parse_args::<syn::Ident>().is_ok_and(|id| id == "test")
}

fn path_attribute(attr: &syn::Attribute) -> Option<String> {
    match (&attr.style, &attr.meta) {
        (syn::AttrStyle::Outer, syn::Meta::NameValue(meta)) if meta.path.is_ident("path") => {
            match &meta.value {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(value),
                    ..
                }) => Some(value.value()),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(super) fn may_import_test_attribute(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Path(path) => may_import_test_attribute(&path.tree),
        syn::UseTree::Name(name) => name.ident.unraw() == "test",
        syn::UseTree::Rename(rename) => rename.rename.unraw() == "test",
        syn::UseTree::Glob(_) => true,
        syn::UseTree::Group(group) => group.items.iter().any(may_import_test_attribute),
    }
}

/// `use super::*;`: the one glob the adapter resolves, by checking the parent
/// scope it imports from.
fn super_glob(tree: &syn::UseTree) -> bool {
    matches!(tree, syn::UseTree::Path(path)
        if path.ident == "super" && matches!(*path.tree, syn::UseTree::Glob(_)))
}

fn item_ident(item: &syn::Item) -> Option<&syn::Ident> {
    match item {
        syn::Item::Const(i) => Some(&i.ident),
        syn::Item::Enum(i) => Some(&i.ident),
        syn::Item::ExternCrate(i) => Some(i.rename.as_ref().map_or(&i.ident, |(_, r)| r)),
        syn::Item::Fn(i) => Some(&i.sig.ident),
        syn::Item::Macro(i) => i.ident.as_ref(),
        syn::Item::Mod(i) => Some(&i.ident),
        syn::Item::Static(i) => Some(&i.ident),
        syn::Item::Struct(i) => Some(&i.ident),
        syn::Item::Trait(i) => Some(&i.ident),
        syn::Item::TraitAlias(i) => Some(&i.ident),
        syn::Item::Type(i) => Some(&i.ident),
        syn::Item::Union(i) => Some(&i.ident),
        _ => None,
    }
}

/// Reject any scope that could replace the built-in test attribute. The
/// adapter does not expand macros or resolve arbitrary imports; `use
/// super::*;` is followed into the parent scope, which must itself be clean
/// and must not name anything `test` that the glob would bring in.
fn check_scope(stack: &[Scope], index: usize, globbed: bool) -> Result<(), String> {
    let shadowed = || Err("test owner scope may shadow the built-in test attribute".into());
    for item in &stack[index].items {
        if globbed && item_ident(item).is_some_and(|ident| ident.unraw() == "test") {
            return shadowed();
        }
        match item {
            syn::Item::Use(item) if super_glob(&item.tree) => {
                if index == 0 {
                    return shadowed();
                }
                check_scope(stack, index - 1, true)?;
            }
            syn::Item::Use(item) if may_import_test_attribute(&item.tree) => return shadowed(),
            syn::Item::Macro(item) if item.ident.is_none() => return shadowed(),
            syn::Item::ExternCrate(item)
                if item.attrs.iter().any(|a| a.path().is_ident("macro_use")) =>
            {
                return shadowed()
            }
            syn::Item::Mod(item) if item.attrs.iter().any(|a| a.path().is_ident("macro_use")) => {
                return shadowed()
            }
            _ => {}
        }
    }
    Ok(())
}

/// The selected function is an unconditional top-level test of the scope.
fn unconditional_function(items: &[syn::Item], function: &str) -> Result<(), String> {
    let functions: Vec<_> = items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(item) if item.sig.ident == function => Some(item),
            _ => None,
        })
        .collect();
    let is_test = |attr: &syn::Attribute| {
        matches!(attr.style, syn::AttrStyle::Outer)
            && matches!(&attr.meta, syn::Meta::Path(path) if path.is_ident("test"))
    };
    let [selected] = functions.as_slice() else {
        return Err("test owner function is not an unconditional test".into());
    };
    if selected.attrs.iter().filter(|a| is_test(a)).count() != 1
        || selected
            .attrs
            .iter()
            .any(|a| !a.path().is_ident("doc") && !is_test(a))
    {
        return Err("test owner function is not an unconditional test".into());
    }
    Ok(())
}

/// A lint level such as the library's `#![warn(missing_docs)]` changes
/// diagnostics, never which items compile, so a crate root may carry one.
fn lint_level(attr: &syn::Attribute) -> bool {
    ["allow", "warn", "deny", "forbid", "expect"]
        .iter()
        .any(|level| attr.path().is_ident(level))
}

/// Resolve `module` from `root` and require `function` to be an
/// unconditional test there. Returns the file holding the final module, which
/// is the owner's source location.
pub(super) fn resolve(
    root: &str,
    module: &str,
    function: &str,
    library: bool,
    read: &dyn Fn(&str) -> Option<String>,
) -> Result<String, String> {
    if !valid_name(function) {
        return Err("invalid test owner selector".into());
    }
    let segments = module_segments(module)?;
    let text = read(root).ok_or("test owner crate root is missing")?;
    let file = syn::parse_file(&text).map_err(|e| format!("invalid test binary source: {e}"))?;
    if file
        .attrs
        .iter()
        .any(|attr| !attr.path().is_ident("doc") && !lint_level(attr))
    {
        return Err("test binary contains conditional attributes".into());
    }
    let mut stack = vec![Scope {
        items: file.items,
        file: root.into(),
        dir: parent(root).into(),
        inline: false,
    }];
    let mut test_only = false;
    for segment in segments {
        let current = stack.last().unwrap();
        let modules: Vec<_> = current
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Mod(item) if item.ident == segment => Some(item),
                _ => None,
            })
            .collect();
        let [declaration] = modules.as_slice() else {
            return Err("test owner module is not compiled".into());
        };
        let cfgs = declaration.attrs.iter().filter(|a| cfg_test(a)).count();
        let paths: Vec<_> = declaration
            .attrs
            .iter()
            .filter_map(path_attribute)
            .collect();
        if cfgs > 1
            || paths.len() > 1
            || declaration.attrs.iter().any(|attr| {
                !(attr.path().is_ident("doc") || cfg_test(attr) || path_attribute(attr).is_some())
            })
        {
            return Err("test owner module has unsupported attributes".into());
        }
        test_only |= cfgs == 1;
        let scope = if let Some((_, items)) = &declaration.content {
            if !paths.is_empty() {
                return Err("test owner module has unsupported attributes".into());
            }
            Scope {
                items: items.clone(),
                file: current.file.clone(),
                dir: format!("{}/{segment}", current.dir),
                inline: true,
            }
        } else {
            let path = match paths.first() {
                // Rust resolves an inline block's `#[path]` against its own
                // nested directory; the adapter does not follow that form.
                Some(_) if current.inline => {
                    return Err("test owner module has unsupported attributes".into())
                }
                Some(path) => join(parent(&current.file), path)?,
                None => {
                    let present: Vec<_> = [
                        format!("{}/{segment}.rs", current.dir),
                        format!("{}/{segment}/mod.rs", current.dir),
                    ]
                    .into_iter()
                    .filter(|path| read(path).is_some())
                    .collect();
                    match <[String; 1]>::try_from(present) {
                        Ok([path]) => path,
                        Err(_) => return Err("test owner module is not compiled".into()),
                    }
                }
            };
            let text = read(&path).ok_or("test owner module is not compiled")?;
            let dir = if paths.is_empty() {
                format!("{}/{segment}", current.dir)
            } else {
                // A `#[path]` file owns its directory, like `mod.rs`.
                parent(&path).into()
            };
            Scope {
                items: parse_file(&path, &text)?.items,
                file: path,
                dir,
                inline: false,
            }
        };
        stack.push(scope);
    }
    if library && !test_only {
        return Err("library test owner module must use exactly cfg(test)".into());
    }
    let last = stack.len() - 1;
    let source = stack[last].file.clone();
    check_scope(&stack, 0, false)?;
    // Every scope from the source file down to the selected module.
    let first = stack.iter().rposition(|s| !s.inline).unwrap();
    for index in first..=last {
        check_scope(&stack, index, false)?;
    }
    unconditional_function(&stack[last].items, function)?;
    Ok(source)
}

/// An in-memory tree for adapter tests.
#[cfg(test)]
pub(super) fn memory(files: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let files: std::collections::BTreeMap<String, String> = files
        .iter()
        .map(|(path, text)| (path.to_string(), text.to_string()))
        .collect();
    move |path: &str| files.get(path).cloned()
}
