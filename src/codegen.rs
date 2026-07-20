//! AST → WebAssembly module bytes, via wasm-encoder.
//!
//! Emits a single exported function `run(<params: f64...>) -> f64`.
//! All variables are f64 locals (params first, then `let`s in source order).
//! Comparisons compile to native f64 comparison (i32 result); in value
//! position the i32 is converted back to f64 (1.0/0.0), in `while`
//! condition position it is used directly.
//!
//! Host capabilities are WASM imports (`env.<name>`) — the generated module
//! can only ever call what the embedder grants at instantiation. The import
//! table IS the capability list (§16 of the architecture doc).

use crate::parser::{BinOp, Expr, Program, Stmt};
use std::collections::HashMap;
use wasm_encoder::{
    BlockType, CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection,
    GlobalSection, GlobalType, ImportSection, Instruction, MemArg, MemorySection, MemoryType,
    Module, TypeSection, ValType,
};

/// A host function importable by generated modules, as `env.<name>`.
#[derive(Clone, Copy)]
pub struct HostFn {
    pub name: &'static str,
    pub n_args: u32,
    pub returns: bool,
}

/// Optional substrate grants for a generated module. Both default to off —
/// a plain `compile_with` module is byte-identical to the pre-opts encoder.
#[derive(Clone, Copy, Default)]
pub struct CompileOpts {
    /// Fuel budget per `run()` call. When set, every loop iteration burns one
    /// unit; hitting zero traps (`unreachable`) instead of hanging the thread.
    /// The remaining fuel is exported as the mutable i32 global `"fuel"`, so
    /// the host can read per-call consumption. Measured tax ≈0% on the benchmark kernel (see README).
    pub fuel: Option<u32>,
    /// Linear-memory grant, in 64KiB pages (min = max — the cell cannot grow
    /// it). Enables the `load(i)` / `store(i, v)` builtins over f64 slots
    /// (slot i = byte offset i*8, explicitly bounds-checked → trap, never
    /// aliasing). The memory is the module's OWN and is exported as `"mem"` —
    /// importing memory from the host stays forbidden by the audit.
    pub memory_pages: Option<u32>,
}

/// Benchmark signature (index.html): `run(n) -> f64`, no capabilities.
pub fn compile(prog: &Program) -> Result<Vec<u8>, String> {
    compile_with(prog, &["n"], &[])
}

/// Canvas kernel signature (canvas.html): `run(t, i, hx, hy) -> hue`,
/// capabilities: sin(x), cos(x), out(x, y).
pub const KERNEL_PARAMS: [&str; 4] = ["t", "i", "hx", "hy"];
pub const KERNEL_IMPORTS: [HostFn; 3] = [
    HostFn { name: "sin", n_args: 1, returns: true },
    HostFn { name: "cos", n_args: 1, returns: true },
    HostFn { name: "out", n_args: 2, returns: false },
];

pub fn compile_kernel(prog: &Program) -> Result<Vec<u8>, String> {
    compile_with(prog, &KERNEL_PARAMS, &KERNEL_IMPORTS)
}

struct Ctx<'a> {
    locals: HashMap<String, u32>,
    imports: &'a [HostFn],
    /// Two reserved scratch locals (used to synthesize `%`: a - trunc(a/b)*b, avoiding re-evaluation).
    scratch0: u32,
    /// i32 scratch local for memory addressing (only allocated when memory is granted).
    scratch_i32: u32,
    /// Fuel metering on: loop headers burn 1 unit/iteration from global 0, trap at zero.
    fuel: bool,
    /// Number of f64 slots addressable via load/store (0 = memory not granted).
    mem_slots: u32,
}

/// Built-in math functions (native WASM instructions — they cost no import-table slot and work under any ABI).
fn builtin_of(name: &str) -> Option<(u32, Instruction<'static>)> {
    Some(match name {
        "min" => (2, Instruction::F64Min),
        "max" => (2, Instruction::F64Max),
        "abs" => (1, Instruction::F64Abs),
        "sqrt" => (1, Instruction::F64Sqrt),
        "floor" => (1, Instruction::F64Floor),
        _ => return None,
    })
}

pub fn compile_with(
    prog: &Program,
    params: &[&str],
    imports: &[HostFn],
) -> Result<Vec<u8>, String> {
    compile_with_opts(prog, params, imports, CompileOpts::default())
}

pub fn compile_with_opts(
    prog: &Program,
    params: &[&str],
    imports: &[HostFn],
    opts: CompileOpts,
) -> Result<Vec<u8>, String> {
    // Local layout: params 0..p, then every `let` in source order (flat scope).
    let mut locals: HashMap<String, u32> = HashMap::new();
    for (k, p) in params.iter().enumerate() {
        locals.insert((*p).to_string(), k as u32);
    }
    let n_params = params.len();
    collect_lets(&prog.stmts, &mut locals)?;
    let n_extra_locals = (locals.len() - n_params) as u32;

    let mut module = Module::new();

    // Types: one per import (indices 0..n), then the kernel's own type (index n).
    let mut types = TypeSection::new();
    for imp in imports {
        let args = vec![ValType::F64; imp.n_args as usize];
        let rets: Vec<ValType> = if imp.returns {
            vec![ValType::F64]
        } else {
            vec![]
        };
        types.ty().function(args, rets);
    }
    let kernel_ty = imports.len() as u32;
    types
        .ty()
        .function(vec![ValType::F64; n_params], [ValType::F64]);
    module.section(&types);

    // Imports occupy function indices 0..n; the kernel is index n.
    if !imports.is_empty() {
        let mut imp_sec = ImportSection::new();
        for (k, imp) in imports.iter().enumerate() {
            imp_sec.import("env", imp.name, EntityType::Function(k as u32));
        }
        module.section(&imp_sec);
    }

    let mut funcs = FunctionSection::new();
    funcs.function(kernel_ty);
    module.section(&funcs);

    // The cell's OWN memory (never imported): min = max, so it can't grow.
    if let Some(pages) = opts.memory_pages {
        let mut mem = MemorySection::new();
        mem.memory(MemoryType {
            minimum: pages as u64,
            maximum: Some(pages as u64),
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
        module.section(&mem);
    }

    // Fuel counter: one mutable i32 global, reset in the prologue each call.
    if opts.fuel.is_some() {
        let mut globals = GlobalSection::new();
        globals.global(
            GlobalType { val_type: ValType::I32, mutable: true, shared: false },
            &wasm_encoder::ConstExpr::i32_const(0),
        );
        module.section(&globals);
    }

    let kernel_idx = imports.len() as u32;
    let mut exports = ExportSection::new();
    exports.export("run", ExportKind::Func, kernel_idx);
    if opts.memory_pages.is_some() {
        exports.export("mem", ExportKind::Memory, 0);
    }
    if opts.fuel.is_some() {
        exports.export("fuel", ExportKind::Global, 0);
    }
    module.section(&exports);

    let scratch0 = locals.len() as u32;
    let ctx = Ctx {
        locals,
        imports,
        scratch0,
        scratch_i32: scratch0 + 2,
        fuel: opts.fuel.is_some(),
        mem_slots: opts.memory_pages.map_or(0, |p| p * 8192), // 64KiB page / 8 bytes per f64
    };
    let mut local_decls: Vec<(u32, ValType)> = vec![(n_extra_locals + 2, ValType::F64)];
    if opts.memory_pages.is_some() {
        local_decls.push((1, ValType::I32));
    }
    let mut f = Function::new(local_decls);
    if let Some(budget) = opts.fuel {
        // Per-call budget reset: a fresh tank every run().
        f.instruction(&Instruction::I32Const(budget as i32));
        f.instruction(&Instruction::GlobalSet(0));
    }
    for s in &prog.stmts {
        emit_stmt(s, &ctx, &mut f)?;
    }
    emit_expr_f64(&prog.ret, &ctx, &mut f)?;
    f.instruction(&Instruction::End);

    let mut code = CodeSection::new();
    code.function(&f);
    module.section(&code);

    Ok(module.finish())
}

fn collect_lets(stmts: &[Stmt], locals: &mut HashMap<String, u32>) -> Result<(), String> {
    for s in stmts {
        match s {
            Stmt::Let(name, _) => {
                if locals.contains_key(name) {
                    return Err(format!(
                        "duplicate 'let {name}' (the PoC DSL has one flat scope)"
                    ));
                }
                let idx = locals.len() as u32;
                locals.insert(name.clone(), idx);
            }
            Stmt::While(_, body) => collect_lets(body, locals)?,
            Stmt::If(_, then_body, else_body) => {
                collect_lets(then_body, locals)?;
                collect_lets(else_body, locals)?;
            }
            Stmt::Assign(_, _) | Stmt::Call(_, _) => {}
        }
    }
    Ok(())
}

fn local_of(name: &str, ctx: &Ctx) -> Result<u32, String> {
    ctx.locals.get(name).copied().ok_or_else(|| {
        format!("undefined variable '{name}' (only parameters and 'let'-declared names exist)")
    })
}

fn import_of(name: &str, ctx: &Ctx) -> Result<(u32, HostFn), String> {
    ctx.imports
        .iter()
        .enumerate()
        .find(|(_, h)| h.name == name)
        .map(|(k, h)| (k as u32, *h))
        .ok_or_else(|| {
            format!(
                "unknown function '{name}' — granted capabilities: [{}]",
                ctx.imports
                    .iter()
                    .map(|h| h.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

/// Emit an f64 slot index as a bounds-checked byte address (i32) on the stack.
/// trunc traps on NaN/negative/huge; the explicit `< mem_slots` check prevents
/// the shift-left-3 wrap that could otherwise alias low slots.
fn emit_mem_addr(idx: &Expr, ctx: &Ctx, f: &mut Function) -> Result<(), String> {
    emit_expr_f64(idx, ctx, f)?;
    f.instruction(&Instruction::I32TruncF64U);
    f.instruction(&Instruction::LocalSet(ctx.scratch_i32));
    f.instruction(&Instruction::LocalGet(ctx.scratch_i32));
    f.instruction(&Instruction::I32Const(ctx.mem_slots as i32));
    f.instruction(&Instruction::I32GeU);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::Unreachable); // out-of-bounds slot → trap
    f.instruction(&Instruction::End);
    f.instruction(&Instruction::LocalGet(ctx.scratch_i32));
    f.instruction(&Instruction::I32Const(3));
    f.instruction(&Instruction::I32Shl); // slot → byte offset (×8)
    Ok(())
}

const MEMARG_F64: MemArg = MemArg { offset: 0, align: 3, memory_index: 0 };

fn emit_call(
    name: &str,
    args: &[Expr],
    ctx: &Ctx,
    f: &mut Function,
) -> Result<HostFn, String> {
    // Memory builtins — only exist when the memory capability was granted.
    if name == "load" || name == "store" {
        if ctx.mem_slots == 0 {
            return Err(format!(
                "'{name}' requires the memory capability (not granted to this cell)"
            ));
        }
        if name == "load" {
            if args.len() != 1 {
                return Err(format!("'load' expects 1 argument (slot index), got {}", args.len()));
            }
            emit_mem_addr(&args[0], ctx, f)?;
            f.instruction(&Instruction::F64Load(MEMARG_F64));
            return Ok(HostFn { name: "load", n_args: 1, returns: true });
        }
        if args.len() != 2 {
            return Err(format!("'store' expects 2 arguments (slot index, value), got {}", args.len()));
        }
        emit_mem_addr(&args[0], ctx, f)?;
        emit_expr_f64(&args[1], ctx, f)?;
        f.instruction(&Instruction::F64Store(MEMARG_F64));
        return Ok(HostFn { name: "store", n_args: 2, returns: false });
    }
    // Built-ins take priority (min/max/abs/sqrt/floor): native instructions, always returning a value.
    if let Some((n_args, instr)) = builtin_of(name) {
        if args.len() as u32 != n_args {
            return Err(format!(
                "builtin '{name}' expects {n_args} argument(s), got {}",
                args.len()
            ));
        }
        for a in args {
            emit_expr_f64(a, ctx, f)?;
        }
        f.instruction(&instr);
        return Ok(HostFn { name: "builtin", n_args, returns: true });
    }
    let (idx, hf) = import_of(name, ctx)?;
    if args.len() as u32 != hf.n_args {
        return Err(format!(
            "'{name}' expects {} argument(s), got {}",
            hf.n_args,
            args.len()
        ));
    }
    for a in args {
        emit_expr_f64(a, ctx, f)?;
    }
    f.instruction(&Instruction::Call(idx));
    Ok(hf)
}

fn emit_stmt(s: &Stmt, ctx: &Ctx, f: &mut Function) -> Result<(), String> {
    match s {
        Stmt::Let(name, e) | Stmt::Assign(name, e) => {
            let idx = local_of(name, ctx)?;
            emit_expr_f64(e, ctx, f)?;
            f.instruction(&Instruction::LocalSet(idx));
        }
        Stmt::While(cond, body) => {
            // block $exit { loop $top { fuel?; if !cond br $exit; body; br $top } }
            f.instruction(&Instruction::Block(BlockType::Empty));
            f.instruction(&Instruction::Loop(BlockType::Empty));
            if ctx.fuel {
                // Burn 1 unit per iteration; an exhausted tank traps instead
                // of hanging the thread — the supervisor catches the trap.
                f.instruction(&Instruction::GlobalGet(0));
                f.instruction(&Instruction::I32Eqz);
                f.instruction(&Instruction::If(BlockType::Empty));
                f.instruction(&Instruction::Unreachable);
                f.instruction(&Instruction::End);
                f.instruction(&Instruction::GlobalGet(0));
                f.instruction(&Instruction::I32Const(1));
                f.instruction(&Instruction::I32Sub);
                f.instruction(&Instruction::GlobalSet(0));
            }
            emit_cond_i32(cond, ctx, f)?;
            f.instruction(&Instruction::I32Eqz);
            f.instruction(&Instruction::BrIf(1));
            for s in body {
                emit_stmt(s, ctx, f)?;
            }
            f.instruction(&Instruction::Br(0));
            f.instruction(&Instruction::End); // loop
            f.instruction(&Instruction::End); // block
        }
        Stmt::Call(name, args) => {
            let hf = emit_call(name, args, ctx, f)?;
            if hf.returns {
                f.instruction(&Instruction::Drop);
            }
        }
        Stmt::If(cond, then_body, else_body) => {
            emit_cond_i32(cond, ctx, f)?;
            f.instruction(&Instruction::If(BlockType::Empty));
            for s in then_body {
                emit_stmt(s, ctx, f)?;
            }
            if !else_body.is_empty() {
                f.instruction(&Instruction::Else);
                for s in else_body {
                    emit_stmt(s, ctx, f)?;
                }
            }
            f.instruction(&Instruction::End);
        }
    }
    Ok(())
}

/// Emit `e` leaving an f64 on the stack.
fn emit_expr_f64(e: &Expr, ctx: &Ctx, f: &mut Function) -> Result<(), String> {
    match e {
        Expr::Num(v) => {
            f.instruction(&Instruction::F64Const((*v).into()));
        }
        Expr::Var(name) => {
            let idx = local_of(name, ctx)?;
            f.instruction(&Instruction::LocalGet(idx));
        }
        Expr::Binary(l, op, r) => match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                emit_expr_f64(l, ctx, f)?;
                emit_expr_f64(r, ctx, f)?;
                f.instruction(match op {
                    BinOp::Add => &Instruction::F64Add,
                    BinOp::Sub => &Instruction::F64Sub,
                    BinOp::Mul => &Instruction::F64Mul,
                    BinOp::Div => &Instruction::F64Div,
                    _ => unreachable!(),
                });
            }
            BinOp::Rem => {
                // fmod semantics (same as JS %): a - trunc(a/b)*b; scratch locals avoid re-evaluation.
                let (s0, s1) = (ctx.scratch0, ctx.scratch0 + 1);
                emit_expr_f64(l, ctx, f)?;
                f.instruction(&Instruction::LocalSet(s0));
                emit_expr_f64(r, ctx, f)?;
                f.instruction(&Instruction::LocalSet(s1));
                f.instruction(&Instruction::LocalGet(s0));
                f.instruction(&Instruction::LocalGet(s0));
                f.instruction(&Instruction::LocalGet(s1));
                f.instruction(&Instruction::F64Div);
                f.instruction(&Instruction::F64Trunc);
                f.instruction(&Instruction::LocalGet(s1));
                f.instruction(&Instruction::F64Mul);
                f.instruction(&Instruction::F64Sub);
            }
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                emit_cmp_i32(l, *op, r, ctx, f)?;
                f.instruction(&Instruction::F64ConvertI32U); // 1.0 / 0.0
            }
        },
        Expr::Call(name, args) => {
            let hf = emit_call(name, args, ctx, f)?;
            if !hf.returns {
                return Err(format!(
                    "'{name}(...)' returns no value and cannot be used in an expression"
                ));
            }
        }
    }
    Ok(())
}

/// Emit `e` leaving an i32 truth value on the stack (for `while` conditions).
fn emit_cond_i32(e: &Expr, ctx: &Ctx, f: &mut Function) -> Result<(), String> {
    if let Expr::Binary(l, op @ (BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge), r) = e {
        emit_cmp_i32(l, *op, r, ctx, f)
    } else {
        // arbitrary f64 expression: truthy = != 0.0
        emit_expr_f64(e, ctx, f)?;
        f.instruction(&Instruction::F64Const(0.0.into()));
        f.instruction(&Instruction::F64Ne);
        Ok(())
    }
}

fn emit_cmp_i32(l: &Expr, op: BinOp, r: &Expr, ctx: &Ctx, f: &mut Function) -> Result<(), String> {
    emit_expr_f64(l, ctx, f)?;
    emit_expr_f64(r, ctx, f)?;
    f.instruction(match op {
        BinOp::Lt => &Instruction::F64Lt,
        BinOp::Gt => &Instruction::F64Gt,
        BinOp::Le => &Instruction::F64Le,
        BinOp::Ge => &Instruction::F64Ge,
        _ => unreachable!(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;

    const KERNEL: &str = "let sum = 0.0;\nlet i = 0.0;\nwhile i < n {\n sum = sum + i * i - sum / (i + 1.0);\n i = i + 1.0;\n}\nsum";

    const CANVAS_KERNEL: &str = "let a = t * 0.5 + i * 0.618;\nlet c = cos(a / 200.0);\nlet s = sin(a / 200.0);\nlet x = 30.0;\nlet y = 0.0;\nlet k = 0.0;\nwhile k < 200.0 {\n let nx = x * c - y * s;\n y = x * s + y * c;\n x = nx;\n k = k + 1.0;\n}\nout(hx + x, hy + y);\na * 0.159";

    #[test]
    fn emits_valid_wasm_module() {
        let bytes = super::compile(&parse(KERNEL).unwrap()).unwrap();
        assert_eq!(&bytes[0..4], b"\0asm");
        wasmparser::validate(&bytes).expect("generated module must validate");
    }

    #[test]
    fn comparison_in_value_position_validates() {
        let bytes =
            super::compile(&parse("let x = 1.0; (x < n) * 2.0 + (x >= 0.5)").unwrap()).unwrap();
        wasmparser::validate(&bytes).unwrap();
    }

    #[test]
    fn kernel_with_imports_validates() {
        let bytes = super::compile_kernel(&parse(CANVAS_KERNEL).unwrap()).unwrap();
        wasmparser::validate(&bytes).expect("kernel module must validate");
    }

    #[test]
    fn unknown_capability_rejected() {
        // fetch() is not in the import table — the cell has no such capability.
        assert!(super::compile_kernel(&parse("fetch(1.0)").unwrap()).is_err());
    }

    #[test]
    fn void_call_in_expression_rejected() {
        assert!(super::compile_kernel(&parse("out(1.0, 2.0) + 1.0").unwrap()).is_err());
    }

    #[test]
    fn arity_mismatch_rejected() {
        assert!(super::compile_kernel(&parse("sin(1.0, 2.0)").unwrap()).is_err());
    }

    #[test]
    fn if_else_and_builtins_validate() {
        let src = "let x = 0.0;\nif n % 2.0 < 1.0 { x = min(n, 10.0); } else { x = max(sqrt(n), abs(0.0 - n)); }\nfloor(x)";
        let bytes = super::compile(&parse(src).unwrap()).unwrap();
        wasmparser::validate(&bytes).expect("if/else + builtins must validate");
    }

    #[test]
    fn builtin_arity_rejected() {
        assert!(super::compile(&parse("min(1.0)").unwrap()).is_err());
    }

    #[test]
    fn undefined_var_rejected() {
        assert!(super::compile(&parse("y + 1.0").unwrap()).is_err());
    }

    #[test]
    fn duplicate_let_rejected() {
        assert!(super::compile(&parse("let a = 1.0; let a = 2.0; a").unwrap()).is_err());
    }

    // ---- CompileOpts: fuel + memory ----

    fn opts(fuel: Option<u32>, pages: Option<u32>) -> super::CompileOpts {
        super::CompileOpts { fuel, memory_pages: pages }
    }

    #[test]
    fn fuel_module_validates_and_exports_gauge() {
        let bytes = super::compile_with_opts(
            &parse(KERNEL).unwrap(),
            &["n"],
            &[],
            opts(Some(1_000_000), None),
        )
        .unwrap();
        wasmparser::validate(&bytes).expect("fueled module must validate");
        // Structural: global section present, "fuel" export present.
        let mut has_global = false;
        let mut has_fuel_export = false;
        for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
            match payload.unwrap() {
                wasmparser::Payload::GlobalSection(_) => has_global = true,
                wasmparser::Payload::ExportSection(reader) => {
                    for e in reader {
                        if e.unwrap().name == "fuel" {
                            has_fuel_export = true;
                        }
                    }
                }
                _ => {}
            }
        }
        assert!(has_global && has_fuel_export);
    }

    #[test]
    fn no_fuel_means_byte_identical_to_plain_compile() {
        let plain = super::compile(&parse(KERNEL).unwrap()).unwrap();
        let via_opts = super::compile_with_opts(
            &parse(KERNEL).unwrap(),
            &["n"],
            &[],
            super::CompileOpts::default(),
        )
        .unwrap();
        assert_eq!(plain, via_opts);
    }

    #[test]
    fn memory_module_validates_and_exports_mem() {
        let src = "let i = 0.0;\nlet s = 0.0;\nwhile i < n {\n s = s + load(i);\n i = i + 1.0;\n}\nstore(0.0, s);\ns";
        let bytes = super::compile_with_opts(
            &parse(src).unwrap(),
            &["n"],
            &[],
            opts(Some(100_000), Some(1)),
        )
        .unwrap();
        wasmparser::validate(&bytes).expect("memory module must validate");
        let mut has_mem_export = false;
        for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
            if let wasmparser::Payload::ExportSection(reader) = payload.unwrap() {
                for e in reader {
                    let e = e.unwrap();
                    if e.name == "mem" && e.kind == wasmparser::ExternalKind::Memory {
                        has_mem_export = true;
                    }
                }
            }
        }
        assert!(has_mem_export);
        // And the audit sees no imports at all — own memory is not an import.
        assert!(crate::audit::imports_of(&bytes).unwrap().is_empty());
    }

    #[test]
    fn load_store_rejected_without_memory_grant() {
        let e = super::compile(&parse("load(0.0)").unwrap()).unwrap_err();
        assert!(e.contains("memory capability"), "{e}");
        let e2 = super::compile(&parse("store(0.0, 1.0);\n0.0").unwrap()).unwrap_err();
        assert!(e2.contains("memory capability"), "{e2}");
    }

    #[test]
    fn memory_builtin_arity_rejected() {
        let o = opts(None, Some(1));
        assert!(super::compile_with_opts(&parse("load(0.0, 1.0)").unwrap(), &["n"], &[], o).is_err());
        assert!(super::compile_with_opts(&parse("store(0.0);\n0.0").unwrap(), &["n"], &[], o).is_err());
    }
}
