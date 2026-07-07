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
    ImportSection, Instruction, Module, TypeSection, ValType,
};

/// A host function importable by generated modules, as `env.<name>`.
#[derive(Clone, Copy)]
pub struct HostFn {
    pub name: &'static str,
    pub n_args: u32,
    pub returns: bool,
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
}

pub fn compile_with(
    prog: &Program,
    params: &[&str],
    imports: &[HostFn],
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

    let kernel_idx = imports.len() as u32;
    let mut exports = ExportSection::new();
    exports.export("run", ExportKind::Func, kernel_idx);
    module.section(&exports);

    let ctx = Ctx { locals, imports };
    let mut f = Function::new([(n_extra_locals, ValType::F64)]);
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

fn emit_call(
    name: &str,
    args: &[Expr],
    ctx: &Ctx,
    f: &mut Function,
) -> Result<HostFn, String> {
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
            // block $exit { loop $top { if !cond br $exit; body; br $top } }
            f.instruction(&Instruction::Block(BlockType::Empty));
            f.instruction(&Instruction::Loop(BlockType::Empty));
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
    fn undefined_var_rejected() {
        assert!(super::compile(&parse("y + 1.0").unwrap()).is_err());
    }

    #[test]
    fn duplicate_let_rejected() {
        assert!(super::compile(&parse("let a = 1.0; let a = 2.0; a").unwrap()).is_err());
    }
}
