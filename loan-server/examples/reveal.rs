//! "Ship the .wasm to the client — all the logic is inside it. Isn't that hidden?"
//!
//! This example answers it on the real cell. It compiles the `monthly` amortization cell —
//! exactly the bytes a browser would download in a wasm-binary deployment — writes them to
//! /tmp/monthly.wasm, and then walks the code section straight out of the bytes and prints
//! every instruction. The multiply / loop / divide that IS the amortization formula shows up
//! in plain sight. wasm is a compile target the browser runs, not an encryption: downloaded
//! means possessed, and a possessed binary disassembles.
//!
//! run: cargo run -p loan-server --example reveal

use wasm_jit::{codegen, parser, UI_IMPORTS, UI_PARAMS};
use wasmparser::{Operator, Parser, Payload};

fn main() {
    // the same DSL the server holds — compiled to the same bytes the browser would get.
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../apps/assets/loan_schema.json")).unwrap();
    let src = schema["cells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "monthly")
        .unwrap()["script"]
        .as_str()
        .unwrap();

    let prog = parser::parse(src).unwrap();
    let bytes = codegen::compile_with_opts(
        &prog,
        &UI_PARAMS,
        &UI_IMPORTS,
        codegen::CompileOpts { fuel: Some(200_000), memory_pages: None },
    )
    .unwrap();
    std::fs::write("/tmp/monthly.wasm", &bytes).unwrap();

    println!(
        "compiled `monthly` cell -> {} bytes  (this is exactly what the browser downloads)",
        bytes.len()
    );
    println!("wrote /tmp/monthly.wasm  —  now recovering the logic straight from the binary:\n");

    // walk the function body. we print the arithmetic (the formula) and the control flow
    // (the loop), and just tally the fuel-metering boilerplate so the signal isn't buried.
    let mut arith = 0usize;
    let mut fuel_ops = 0usize;
    let mut step = 0usize;
    for payload in Parser::new(0).parse_all(&bytes) {
        if let Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut r = body.get_operators_reader().unwrap();
            while !r.eof() {
                let op = r.read().unwrap();
                let line = format!("{op:?}");
                // classify: the maths and the loop are the secret; global.get/set + i32 compares
                // around them are the fuel meter — count those instead of printing every one.
                let is_fuel = matches!(
                    op,
                    Operator::GlobalGet { .. }
                        | Operator::GlobalSet { .. }
                        | Operator::I32Const { .. }
                        | Operator::I32Sub
                        | Operator::I32LtS
                        | Operator::I32GeS
                        | Operator::LocalGet { .. }
                        | Operator::LocalSet { .. }
                        | Operator::LocalTee { .. }
                );
                let interesting = matches!(
                    op,
                    Operator::F64Mul
                        | Operator::F64Add
                        | Operator::F64Sub
                        | Operator::F64Div
                        | Operator::F64Const { .. }
                        | Operator::F64Ge
                        | Operator::F64Lt
                        | Operator::Call { .. }
                        | Operator::Loop { .. }
                        | Operator::Block { .. }
                        | Operator::If { .. }
                        | Operator::End
                        | Operator::Br { .. }
                        | Operator::BrIf { .. }
                );
                if is_fuel && !interesting {
                    fuel_ops += 1;
                    continue;
                }
                if matches!(op, Operator::F64Mul | Operator::F64Add | Operator::F64Sub | Operator::F64Div) {
                    arith += 1;
                }
                println!("  {step:>3}  {line}");
                step += 1;
            }
        }
    }
    println!("\n{arith} floating-point ops carry the amortization formula; {fuel_ops} more ops are the fuel meter.");
    println!("try it yourself:  curl -s http://127.0.0.1:8787/... won't give you this (server hides it),");
    println!("but any wasm you SHIP to a browser can be dumped exactly like this.");
}
