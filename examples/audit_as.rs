//! Dogfood:用 wasm-jit 自己的 import 審計驗一顆「真 AssemblyScript 編出來的」種子。
fn main() {
    let bytes = std::fs::read("assemblyscript/build/buddha.wasm")
        .expect("先 `cd assemblyscript && npm run build` 產出 buddha.wasm");
    println!("AS buddha.wasm = {} bytes", bytes.len());
    let imps = wasm_jit::audit::imports_of(&bytes).unwrap();
    println!("import 節({} 個):", imps.len());
    for i in &imps {
        println!("  {}::{}  ({})", i.module, i.name, i.kind);
    }
    let names = ["sin", "cos", "hue", "disc", "ring", "arc", "line"];
    let grants: Vec<_> = names
        .iter()
        .map(|n| wasm_jit::audit::Grant { module: "env", name: n })
        .collect();
    match wasm_jit::audit::audit(&bytes, &grants) {
        Ok(()) => println!("audit(draw grants): ✅ 通過 —— AS 產物走同一道沙箱"),
        Err(e) => println!("audit: ❌ {e}"),
    }
    // 反例:若只授權 sin/cos(不給繪圖能力),同一顆 AS 種子就會被拒
    let tiny: Vec<_> = ["sin", "cos"]
        .iter()
        .map(|n| wasm_jit::audit::Grant { module: "env", name: n })
        .collect();
    match wasm_jit::audit::audit(&bytes, &tiny) {
        Ok(()) => println!("audit(僅 sin/cos): 意外通過"),
        Err(e) => println!("audit(僅 sin/cos): ❌（如預期）{e}"),
    }
}
