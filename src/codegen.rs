//! AST → WebAssembly module bytes, via wasm-encoder.
//!
//! Emits a single exported function `run(n: f64) -> f64`.
//! All variables are f64 locals (param `n` = local 0, `let`s = 1..).
//! Comparisons compile to native f64 comparison (i32 result); in value
//! position the i32 is converted back to f64 (1.0/0.0), in `while`
//! condition position it is used directly.

use crate::parser::{BinOp, Expr, Program, Stmt};
use std::collections::HashMap;
use wasm_encoder::{
    BlockType, CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction,
    Module, TypeSection, ValType,
};

pub fn compile(prog: &Program) -> Result<Vec<u8>, String> {
    // Local layout: n=0, then every `let` in source order (whole program, flat scope).
    let mut locals: HashMap<String, u32> = HashMap::new();
    locals.insert("n".to_string(), 0);
    collect_lets(&prog.stmts, &mut locals)?;
    let n_extra_locals = (locals.len() - 1) as u32;

    let mut module = Module::new();

    let mut types = TypeSection::new();
    types.ty().function([ValType::F64], [ValType::F64]);
    module.section(&types);

    let mut funcs = FunctionSection::new();
    funcs.function(0);
    module.section(&funcs);

    let mut exports = ExportSection::new();
    exports.export("run", ExportKind::Func, 0);
    module.section(&exports);

    let mut f = Function::new([(n_extra_locals, ValType::F64)]);
    for s in &prog.stmts {
        emit_stmt(s, &locals, &mut f)?;
    }
    emit_expr_f64(&prog.ret, &locals, &mut f)?;
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
            Stmt::Assign(_, _) => {}
        }
    }
    Ok(())
}

fn local_of(name: &str, locals: &HashMap<String, u32>) -> Result<u32, String> {
    locals
        .get(name)
        .copied()
        .ok_or_else(|| format!("undefined variable '{name}' (only 'n' and 'let'-declared names exist)"))
}

fn emit_stmt(s: &Stmt, locals: &HashMap<String, u32>, f: &mut Function) -> Result<(), String> {
    match s {
        Stmt::Let(name, e) | Stmt::Assign(name, e) => {
            let idx = local_of(name, locals)?;
            emit_expr_f64(e, locals, f)?;
            f.instruction(&Instruction::LocalSet(idx));
        }
        Stmt::While(cond, body) => {
            // block $exit { loop $top { if !cond br $exit; body; br $top } }
            f.instruction(&Instruction::Block(BlockType::Empty));
            f.instruction(&Instruction::Loop(BlockType::Empty));
            emit_cond_i32(cond, locals, f)?;
            f.instruction(&Instruction::I32Eqz);
            f.instruction(&Instruction::BrIf(1));
            for s in body {
                emit_stmt(s, locals, f)?;
            }
            f.instruction(&Instruction::Br(0));
            f.instruction(&Instruction::End); // loop
            f.instruction(&Instruction::End); // block
        }
    }
    Ok(())
}

/// Emit `e` leaving an f64 on the stack.
fn emit_expr_f64(e: &Expr, locals: &HashMap<String, u32>, f: &mut Function) -> Result<(), String> {
    match e {
        Expr::Num(v) => {
            f.instruction(&Instruction::F64Const((*v).into()));
        }
        Expr::Var(name) => {
            let idx = local_of(name, locals)?;
            f.instruction(&Instruction::LocalGet(idx));
        }
        Expr::Binary(l, op, r) => match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                emit_expr_f64(l, locals, f)?;
                emit_expr_f64(r, locals, f)?;
                f.instruction(match op {
                    BinOp::Add => &Instruction::F64Add,
                    BinOp::Sub => &Instruction::F64Sub,
                    BinOp::Mul => &Instruction::F64Mul,
                    BinOp::Div => &Instruction::F64Div,
                    _ => unreachable!(),
                });
            }
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                emit_cmp_i32(l, *op, r, locals, f)?;
                f.instruction(&Instruction::F64ConvertI32U); // 1.0 / 0.0
            }
        },
    }
    Ok(())
}

/// Emit `e` leaving an i32 truth value on the stack (for `while` conditions).
fn emit_cond_i32(e: &Expr, locals: &HashMap<String, u32>, f: &mut Function) -> Result<(), String> {
    if let Expr::Binary(l, op @ (BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge), r) = e {
        emit_cmp_i32(l, *op, r, locals, f)
    } else {
        // arbitrary f64 expression: truthy = != 0.0
        emit_expr_f64(e, locals, f)?;
        f.instruction(&Instruction::F64Const(0.0.into()));
        f.instruction(&Instruction::F64Ne);
        Ok(())
    }
}

fn emit_cmp_i32(
    l: &Expr,
    op: BinOp,
    r: &Expr,
    locals: &HashMap<String, u32>,
    f: &mut Function,
) -> Result<(), String> {
    emit_expr_f64(l, locals, f)?;
    emit_expr_f64(r, locals, f)?;
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

    #[test]
    fn emits_valid_wasm_module() {
        let bytes = super::compile(&parse(KERNEL).unwrap()).unwrap();
        assert_eq!(&bytes[0..4], b"\0asm");
        wasmparser::validate(&bytes).expect("generated module must validate");
    }

    #[test]
    fn comparison_in_value_position_validates() {
        let bytes = super::compile(&parse("let x = 1.0; (x < n) * 2.0 + (x >= 0.5)").unwrap())
            .unwrap();
        wasmparser::validate(&bytes).unwrap();
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
