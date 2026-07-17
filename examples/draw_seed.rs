//! Compile a `draw` DSL seed with the REAL wasm-jit pipeline (parse + codegen),
//! audit it against the draw capability grants (proving imports ⊆ primitives),
//! and emit the module bytes as base64 for inlining into a self-contained page.
//!
//!   cargo run --release --example draw_seed -- examples/indranet.dsl
//!
//! stderr: byte count + audit verdict; stdout: base64 of the .wasm module.

use std::fs;
use wasm_jit::audit::{self, Grant};
use wasm_jit::codegen::{self, HostFn};
use wasm_jit::parser;

fn b64(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: draw_seed <seed.dsl>");
    let src = fs::read_to_string(&path).expect("read seed");

    // The exact draw ABI (mirrors compile_draw_wasm in src/lib.rs).
    let params: [&str; 3] = ["t", "w", "h"];
    let imports = [
        HostFn { name: "sin", n_args: 1, returns: true },
        HostFn { name: "cos", n_args: 1, returns: true },
        HostFn { name: "hue", n_args: 1, returns: false },
        HostFn { name: "rgb", n_args: 3, returns: false },
        HostFn { name: "hsl", n_args: 3, returns: false },
        HostFn { name: "disc", n_args: 3, returns: false },
        HostFn { name: "ring", n_args: 3, returns: false },
        HostFn { name: "arc", n_args: 5, returns: false },
        HostFn { name: "line", n_args: 4, returns: false },
    ];

    let prog = match parser::parse(&src) {
        Ok(p) => p,
        Err(e) => { eprintln!("PARSE ERROR: {e}"); std::process::exit(1); }
    };
    let bytes = match codegen::compile_with(&prog, &params, &imports) {
        Ok(b) => b,
        Err(e) => { eprintln!("CODEGEN ERROR: {e}"); std::process::exit(1); }
    };

    // The sandbox proof: every import ⊆ the 9 drawing primitives (no fetch, no state).
    let grants: Vec<Grant> = ["sin", "cos", "hue", "rgb", "hsl", "disc", "ring", "arc", "line"]
        .iter().map(|n| Grant { module: "env", name: n }).collect();
    match audit::audit(&bytes, &grants) {
        Ok(()) => eprintln!("compiled {} bytes; audit ✓ imports ⊆ {{9 primitives}} — cannot fetch, cannot touch state", bytes.len()),
        Err(e) => { eprintln!("AUDIT FAILED: {e}"); std::process::exit(1); }
    }
    println!("{}", b64(&bytes));
}
