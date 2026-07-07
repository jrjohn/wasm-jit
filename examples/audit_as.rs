//! Dogfood: audit a real AssemblyScript-compiled seed with wasm-jit's own import audit.
fn main() {
    let bytes = std::fs::read("assemblyscript/build/buddha.wasm")
        .expect("run `cd assemblyscript && npm run build` first to produce buddha.wasm");
    println!("AS buddha.wasm = {} bytes", bytes.len());
    let imps = wasm_jit::audit::imports_of(&bytes).unwrap();
    println!("import section ({} entries):", imps.len());
    for i in &imps {
        println!("  {}::{}  ({})", i.module, i.name, i.kind);
    }
    let names = ["sin", "cos", "hue", "disc", "ring", "arc", "line"];
    let grants: Vec<_> = names
        .iter()
        .map(|n| wasm_jit::audit::Grant { module: "env", name: n })
        .collect();
    match wasm_jit::audit::audit(&bytes, &grants) {
        Ok(()) => println!("audit(draw grants): ✅ passed — the AS output goes through the same sandbox"),
        Err(e) => println!("audit: ❌ {e}"),
    }
    // counter-case: granting only sin/cos (no drawing capabilities) rejects the same AS seed
    let tiny: Vec<_> = ["sin", "cos"]
        .iter()
        .map(|n| wasm_jit::audit::Grant { module: "env", name: n })
        .collect();
    match wasm_jit::audit::audit(&bytes, &tiny) {
        Ok(()) => println!("audit(sin/cos only): unexpectedly passed"),
        Err(e) => println!("audit(sin/cos only): ❌ (as expected) {e}"),
    }
}
