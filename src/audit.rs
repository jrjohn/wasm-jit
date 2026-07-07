//! audit.rs — 種子語言光譜的地基:import-section 審計。
//!
//! 洞見:host 的 Cell 不在乎 bytes 是誰編的(自家 DSL / AssemblyScript /
//! Rust→wasm / 手寫 WAT)。安全模型只看一件事:**模組宣告的 imports ⊆
//! host 授予的 capability 清單**。這道審計讓「文法圍欄」升級成「import 節
//! 審計」——語言可以豐富到塞不進 prompt,capability 圍欄一寸不動。
//!
//! 這是 codegen.rs 那條「未授權函式 codegen 即拒」的模組級對應:自家 DSL
//! 在 codegen 就擋掉;外部編的 WASM 在 instantiate 前擋掉。同一道牆,兩個入口。

/// 模組宣告的一個 import(僅取我們在意的欄位)。
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

/// 掃出一個 WASM 模組宣告的全部 imports(用 wasmparser,不 instantiate)。
/// 處理 0.252 的三種 import 分組格式(Single / Compact1 / Compact2)。
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

/// 授予的 capability(module::name 的函式)。
pub struct Grant {
    pub module: &'static str,
    pub name: &'static str,
}

/// 審計:模組的每一個 import 都必須在 grants 裡,且必須是 func。
/// 回傳第一個違規(未授權 import / 非函式 import,如 memory/table/global),
/// None = 通過。這就是「fetch() 被拒」的模組級版本。
pub fn audit(bytes: &[u8], grants: &[Grant]) -> Result<(), String> {
    let imports = imports_of(bytes)?;
    for imp in &imports {
        // 只允許函式 import;memory/table/global import = 想要 host 給它更大的世界,拒。
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

    // canvas kernel 的授權清單
    fn kernel_grants() -> Vec<Grant> {
        vec![
            Grant { module: "env", name: "sin" },
            Grant { module: "env", name: "cos" },
            Grant { module: "env", name: "out" },
        ]
    }

    #[test]
    fn imports_are_read_without_instantiation() {
        // 用到 sin + out 的 kernel → import section 應含這兩個
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
        // 手工組一個 import 了 env::fetch 的模組(模擬外部語言編的越權種子)
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
        // 外部種子想 import host 的 memory(拿更大的世界)→ 拒
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
