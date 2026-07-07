//! audit.rs — the foundation of the seed-language spectrum: import-section auditing.
//!
//! The insight: the host's Cell doesn't care who compiled the bytes (the home
//! DSL / AssemblyScript / Rust→wasm / hand-written WAT). The security model
//! looks at exactly one thing: **the imports a module declares ⊆ the capability
//! list the host grants**. This audit promotes the "grammar fence" into an
//! "import-section audit" — the language can grow rich enough that it no longer
//! fits in a prompt, and the capability fence doesn't move an inch.
//!
//! This is the module-level counterpart to codegen.rs's "unauthorized function
//! is rejected at codegen": the home DSL is blocked at codegen; externally
//! compiled WASM is blocked before instantiation. One wall, two entrances.

/// A single import declared by a module (only the fields we care about).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub module: String,
    pub name: String,
    /// "func" | "table" | "memory" | "global" | "tag"
    pub kind: &'static str,
}

fn kind_of(ty: wasmparser::TypeRef) -> &'static str {
    use wasmparser::TypeRef;
    match ty {
        TypeRef::Func(_) | TypeRef::FuncExact(_) => "func",
        TypeRef::Table(_) => "table",
        TypeRef::Memory(_) => "memory",
        TypeRef::Global(_) => "global",
        TypeRef::Tag(_) => "tag",
    }
}

/// Scan out all imports a WASM module declares (via wasmparser, without instantiating).
/// Handles all three of 0.252's import grouping formats (Single / Compact1 / Compact2).
pub fn imports_of(bytes: &[u8]) -> Result<Vec<Import>, String> {
    use wasmparser::{Imports, Parser, Payload};
    let mut out = Vec::new();
    let mut push = |module: &str, name: &str, ty| {
        out.push(Import {
            module: module.to_string(),
            name: name.to_string(),
            kind: kind_of(ty),
        });
    };
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload.map_err(|e| format!("malformed wasm: {e}"))?;
        if let Payload::ImportSection(reader) = payload {
            for group in reader {
                match group.map_err(|e| format!("bad import: {e}"))? {
                    Imports::Single(_, imp) => push(imp.module, imp.name, imp.ty),
                    Imports::Compact1 { module, items } => {
                        for it in items {
                            let it = it.map_err(|e| format!("bad import: {e}"))?;
                            push(module, it.name, it.ty);
                        }
                    }
                    Imports::Compact2 { module, ty, names } => {
                        for n in names {
                            let n = n.map_err(|e| format!("bad import: {e}"))?;
                            push(module, n, ty);
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

/// A granted capability (the function at module::name).
pub struct Grant {
    pub module: &'static str,
    pub name: &'static str,
}

/// Audit: every import a module declares must be in `grants` and must be a func.
/// Returns the first violation (an unauthorized import / a non-function import such
/// as memory/table/global); Ok(()) = passed. This is the module-level version of
/// "fetch() is rejected".
pub fn audit(bytes: &[u8], grants: &[Grant]) -> Result<(), String> {
    let imports = imports_of(bytes)?;
    for imp in &imports {
        // Only function imports are allowed; a memory/table/global import means it wants the host to hand it a bigger world — reject.
        if imp.kind != "func" {
            return Err(format!(
                "unauthorized {} import '{}::{}' — only host-granted function capabilities are allowed",
                imp.kind, imp.module, imp.name
            ));
        }
        let ok = grants
            .iter()
            .any(|g| g.module == imp.module && g.name == imp.name);
        if !ok {
            let list = grants
                .iter()
                .map(|g| format!("{}::{}", g.module, g.name))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "unauthorized import '{}::{}' — granted capabilities: [{list}]",
                imp.module, imp.name
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{codegen, parser};

    fn cell_bytes(src: &str) -> Vec<u8> {
        codegen::compile_kernel(&parser::parse(src).unwrap()).unwrap()
    }

    // the grant list for the canvas kernel
    fn kernel_grants() -> Vec<Grant> {
        vec![
            Grant { module: "env", name: "sin" },
            Grant { module: "env", name: "cos" },
            Grant { module: "env", name: "out" },
        ]
    }

    #[test]
    fn imports_are_read_without_instantiation() {
        // a kernel using sin + out → its import section should contain both
        let bytes = cell_bytes("let a = sin(t);\nout(hx + a, hy);\na");
        let imps = imports_of(&bytes).unwrap();
        let names: Vec<_> = imps.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"sin"));
        assert!(names.contains(&"out"));
        assert!(imps.iter().all(|i| i.kind == "func" && i.module == "env"));
    }

    #[test]
    fn granted_module_passes_audit() {
        let bytes = cell_bytes("let a = sin(t) + cos(t);\nout(hx + a, hy);\na");
        assert!(audit(&bytes, &kernel_grants()).is_ok());
    }

    #[test]
    fn unauthorized_import_rejected() {
        // hand-build a module that imports env::fetch (simulating an over-reaching seed compiled by an external language)
        use wasm_encoder::{EntityType, ImportSection, Module, TypeSection};
        let mut m = Module::new();
        let mut types = TypeSection::new();
        types.ty().function([], []);
        m.section(&types);
        let mut imp = ImportSection::new();
        imp.import("env", "fetch", EntityType::Function(0));
        m.section(&imp);
        let bytes = m.finish();
        let e = audit(&bytes, &kernel_grants()).unwrap_err();
        assert!(e.contains("fetch"), "{e}");
        assert!(e.contains("granted capabilities"), "{e}");
    }

    #[test]
    fn memory_import_rejected() {
        // an external seed wants to import the host's memory (grabbing a bigger world) → reject
        use wasm_encoder::{EntityType, ImportSection, MemoryType, Module};
        let mut m = Module::new();
        let mut imp = ImportSection::new();
        imp.import(
            "env",
            "memory",
            EntityType::Memory(MemoryType {
                minimum: 1,
                maximum: None,
                memory64: false,
                shared: false,
                page_size_log2: None,
            }),
        );
        m.section(&imp);
        let e = audit(&m.finish(), &kernel_grants()).unwrap_err();
        assert!(e.contains("memory import"), "{e}");
    }
}
