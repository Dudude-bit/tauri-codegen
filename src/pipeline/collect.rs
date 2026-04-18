//! Walks command signatures and returns every struct/enum/alias definition
//! reachable from them. Conflicts (same name from two different source files)
//! and unresolved (macro-generated, probably) types are reported separately
//! so the caller can decide whether to bail or continue.
//!
//! The walk is a fixpoint: start from command args and return types, then
//! follow every nested custom type recorded in the resolver. `processed` and
//! `resolved_types` together prevent cycles and double-counting.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostics::Diagnostics;
use crate::models::{
    walk_custom_type_names, RustEnum, RustStruct, RustType, RustTypeAlias, TauriCommand,
    VariantData,
};
use crate::parser::{parse_types_with_aliases, ParsedTypes};
use crate::resolver::{ModuleResolver, ResolutionResult};

/// Result of type collection with potential conflicts.
pub struct TypeCollectionResult {
    /// Collected structs reachable from command signatures
    pub structs: Vec<RustStruct>,
    /// Collected enums reachable from command signatures
    pub enums: Vec<RustEnum>,
    /// Collected type aliases reachable from command signatures
    pub aliases: Vec<RustTypeAlias>,
    /// Conflicts: type name -> list of conflicting source files
    pub conflicts: HashMap<String, Vec<PathBuf>>,
    /// Unresolved types: type name -> file where it was used
    pub unresolved: HashMap<String, PathBuf>,
}

/// Entry point: walk the command graph, return everything reachable.
pub fn collect_reachable_types(
    commands: &[TauriCommand],
    resolver: &ModuleResolver,
    expanded_types: Option<&ParsedTypes>,
    diag: &Diagnostics,
) -> TypeCollectionResult {
    let mut state = CollectState::new(resolver, diag);
    state.seed_expanded_types(expanded_types);
    state.seed_from_commands(commands);
    state.drain();
    state.finalize_reexport_aliases();
    state.into_result()
}

/// Returns every `Custom(name)` leaf from a `RustType`, sorted and deduped.
fn custom_types(ty: &RustType) -> Vec<String> {
    let mut set: HashSet<String> = HashSet::new();
    walk_custom_type_names(ty, &mut |name| {
        set.insert(name.to_string());
    });
    let mut out: Vec<String> = set.into_iter().collect();
    out.sort();
    out
}

/// Same, but aggregated across every field in a struct variant.
fn custom_types_from_variant(data: &VariantData) -> Vec<String> {
    match data {
        VariantData::Unit => Vec::new(),
        VariantData::Tuple(types) => types.iter().flat_map(custom_types).collect(),
        VariantData::Struct(fields) => fields.iter().flat_map(|f| custom_types(&f.ty)).collect(),
    }
}

/// Mutable state threaded through the fixpoint walk. Collapsing the eight
/// previously-separate `&mut` arguments into one struct retires the
/// too-many-arguments lint exemption.
struct CollectState<'a> {
    resolver: &'a ModuleResolver,
    diag: &'a Diagnostics,

    structs: Vec<RustStruct>,
    enums: Vec<RustEnum>,
    aliases: Vec<RustTypeAlias>,
    conflicts: HashMap<String, Vec<PathBuf>>,
    unresolved: HashMap<String, PathBuf>,

    resolved_types: HashMap<String, PathBuf>,
    parsed_files: HashMap<PathBuf, ParsedTypes>,
    seen_structs: HashSet<(String, PathBuf)>,
    seen_enums: HashSet<(String, PathBuf)>,
    seen_aliases: HashSet<String>,
    reexport_aliases: HashMap<String, (String, PathBuf)>,
    to_process: Vec<(String, PathBuf)>,
    processed: HashSet<(String, PathBuf)>,
}

impl<'a> CollectState<'a> {
    fn new(resolver: &'a ModuleResolver, diag: &'a Diagnostics) -> Self {
        Self {
            resolver,
            diag,
            structs: Vec::new(),
            enums: Vec::new(),
            aliases: Vec::new(),
            conflicts: HashMap::new(),
            unresolved: HashMap::new(),
            resolved_types: HashMap::new(),
            parsed_files: HashMap::new(),
            seen_structs: HashSet::new(),
            seen_enums: HashSet::new(),
            seen_aliases: HashSet::new(),
            reexport_aliases: HashMap::new(),
            to_process: Vec::new(),
            processed: HashSet::new(),
        }
    }

    fn seed_expanded_types(&mut self, expanded_types: Option<&ParsedTypes>) {
        if let Some(parsed) = expanded_types {
            self.parsed_files
                .insert(PathBuf::from("<cargo-expand>"), parsed.clone());
        }
    }

    fn add_conflict_path(&mut self, name: &str, path: &Path) {
        let entry = self.conflicts.entry(name.to_string()).or_default();
        if !entry.iter().any(|p| p == path) {
            entry.push(path.to_path_buf());
        }
    }

    fn resolve_and_enqueue(&mut self, type_name: &str, from_file: &Path) {
        let simple_name = type_name
            .split("::")
            .last()
            .unwrap_or(type_name)
            .to_string();

        match self.resolver.resolve_type(type_name, from_file) {
            ResolutionResult::Found(source) => {
                if let Some(existing) = self.resolved_types.get(&simple_name).cloned() {
                    if existing != source {
                        self.add_conflict_path(&simple_name, &existing);
                        self.add_conflict_path(&simple_name, &source);
                    }
                } else {
                    self.resolved_types
                        .insert(simple_name.clone(), source.clone());
                    self.to_process.push((simple_name, source));
                }
            }
            ResolutionResult::FoundWithAlias(source, original_name) => {
                if let Some(existing) = self.resolved_types.get(&simple_name).cloned() {
                    if existing != source {
                        self.add_conflict_path(&simple_name, &existing);
                        self.add_conflict_path(&simple_name, &source);
                    }
                } else {
                    self.resolved_types
                        .insert(simple_name.clone(), source.clone());
                }
                self.reexport_aliases
                    .entry(simple_name)
                    .or_insert_with(|| (original_name.clone(), source.clone()));
                self.to_process.push((original_name, source));
            }
            ResolutionResult::Ambiguous(paths) => {
                for path in paths {
                    self.add_conflict_path(&simple_name, &path);
                }
            }
            ResolutionResult::NotFound => {
                self.unresolved
                    .entry(simple_name)
                    .or_insert_with(|| from_file.to_path_buf());
            }
        }
    }

    fn seed_from_commands(&mut self, commands: &[TauriCommand]) {
        for cmd in commands {
            let cmd_file = cmd.source_file.clone();
            for arg in &cmd.args {
                for t in custom_types(&arg.ty) {
                    self.resolve_and_enqueue(&t, &cmd_file);
                }
            }
            if let Some(ret_type) = &cmd.return_type {
                for t in custom_types(ret_type) {
                    self.resolve_and_enqueue(&t, &cmd_file);
                }
            }
        }
    }

    /// Lazily read+parse a source file on first reference. Returns `true`
    /// when the file is available in `parsed_files` afterwards.
    fn ensure_parsed(&mut self, type_file: &Path) -> bool {
        if self.parsed_files.contains_key(type_file) {
            return true;
        }
        let content = match fs::read_to_string(type_file) {
            Ok(c) => c,
            Err(e) => {
                self.diag.warn(format!(
                    "Failed to read file for types {}: {}",
                    type_file.display(),
                    e
                ));
                return false;
            }
        };
        match parse_types_with_aliases(&content, type_file) {
            Ok(parsed) => {
                self.parsed_files.insert(type_file.to_path_buf(), parsed);
                true
            }
            Err(e) => {
                self.diag.warn(format!(
                    "Failed to parse types in {}: {}",
                    type_file.display(),
                    e
                ));
                false
            }
        }
    }

    fn drain(&mut self) {
        let expanded_path = PathBuf::from("<cargo-expand>");
        while let Some((type_name, type_file)) = self.to_process.pop() {
            let key = (type_name.clone(), type_file.clone());
            if !self.processed.insert(key) {
                continue;
            }

            if type_file != expanded_path && !self.ensure_parsed(&type_file) {
                continue;
            }

            // Clone the parsed snapshot so we can mutate state below without
            // holding a borrow into `parsed_files` across `resolve_and_enqueue`.
            let parsed = match self.parsed_files.get(&type_file).cloned() {
                Some(p) => p,
                None => continue,
            };

            if let Some(s) = parsed.structs.iter().find(|s| s.name == type_name) {
                if self
                    .seen_structs
                    .insert((s.name.clone(), type_file.clone()))
                {
                    self.structs.push(s.clone());
                }
                for field in &s.fields {
                    for t in custom_types(&field.ty) {
                        self.resolve_and_enqueue(&t, &type_file);
                    }
                }
                continue;
            }

            if let Some(e) = parsed.enums.iter().find(|e| e.name == type_name) {
                if self.seen_enums.insert((e.name.clone(), type_file.clone())) {
                    self.enums.push(e.clone());
                }
                for variant in &e.variants {
                    for t in custom_types_from_variant(&variant.data) {
                        self.resolve_and_enqueue(&t, &type_file);
                    }
                }
                continue;
            }

            if let Some(alias) = parsed.aliases.iter().find(|a| a.name == type_name) {
                if self.seen_aliases.insert(alias.name.clone()) {
                    self.aliases.push(alias.clone());
                }
                let alias_source = alias.source_file.clone();
                for t in custom_types(&alias.target) {
                    self.resolve_and_enqueue(&t, &alias_source);
                }
            }
        }
    }

    fn finalize_reexport_aliases(&mut self) {
        // Drain the re-export map so self is free to mutate `self.aliases`.
        let reexports: Vec<(String, (String, PathBuf))> = self.reexport_aliases.drain().collect();
        for (alias_name, (original_name, source_file)) in reexports {
            if self.seen_aliases.contains(&alias_name) {
                continue;
            }
            let generics = self
                .parsed_files
                .get(&source_file)
                .and_then(|parsed| {
                    parsed
                        .structs
                        .iter()
                        .find(|s| s.name == original_name)
                        .map(|s| s.generics.clone())
                        .or_else(|| {
                            parsed
                                .enums
                                .iter()
                                .find(|e| e.name == original_name)
                                .map(|e| e.generics.clone())
                        })
                })
                .unwrap_or_default();

            self.aliases.push(RustTypeAlias {
                name: alias_name,
                generics,
                target: RustType::Custom(original_name),
                source_file,
            });
        }
    }

    fn into_result(self) -> TypeCollectionResult {
        TypeCollectionResult {
            structs: self.structs,
            enums: self.enums,
            aliases: self.aliases,
            conflicts: self.conflicts,
            unresolved: self.unresolved,
        }
    }
}
