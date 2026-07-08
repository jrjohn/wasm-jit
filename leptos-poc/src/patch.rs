//! patch.rs — the structural patch grammar (docs §18, gate 2).
//!
//! Live manifestation stops being "replace the whole schema": structure
//! changes arrive as incremental patches, validated against the vocabulary
//! before touching the tree. Cells stay pure f64 — they never *carry*
//! structure; they *gate* it: an event may name a patch, and the host applies
//! it only when the cell's verdict is nonzero. Generation never creates
//! vocabulary; patches only compose it.

use serde_json::Value;

/// The LiveUI vocabulary. A patch introducing any other node type is rejected.
pub const VOCAB: [&str; 7] = ["stack", "row", "label", "value", "button", "slider", "input"];

/// Recursive vocabulary check — used for whole trees at load and for every
/// patch node before it may touch the tree.
pub fn validate_node(node: &Value) -> Result<(), String> {
    let t = node
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or("patch node lacks a \"type\"")?;
    if !VOCAB.contains(&t) {
        return Err(format!(
            "node type '{t}' is not in the vocabulary [{}]",
            VOCAB.join(", ")
        ));
    }
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for c in children {
            validate_node(c)?;
        }
    }
    Ok(())
}

/// Walk `path` (child indices) from root; return the parent array and final index.
fn resolve<'a>(
    tree: &'a mut Value,
    path: &[usize],
) -> Result<(&'a mut Vec<Value>, usize), String> {
    if path.is_empty() {
        return Err("empty patch path".into());
    }
    let mut node = tree;
    for (depth, idx) in path[..path.len() - 1].iter().enumerate() {
        let children = node
            .get_mut("children")
            .and_then(|c| c.as_array_mut())
            .ok_or_else(|| format!("path step {depth}: node has no children"))?;
        node = children
            .get_mut(*idx)
            .ok_or_else(|| format!("path step {depth}: index {idx} out of range"))?;
    }
    let last = *path.last().unwrap();
    let children = node
        .get_mut("children")
        .and_then(|c| c.as_array_mut())
        .ok_or("patch target's parent has no children array")?;
    Ok((children, last))
}

fn path_of(patch: &Value) -> Result<Vec<usize>, String> {
    patch
        .get("path")
        .and_then(|p| p.as_array())
        .ok_or("patch lacks a \"path\" array")?
        .iter()
        .map(|v| {
            v.as_u64()
                .map(|n| n as usize)
                .ok_or_else(|| "path elements must be non-negative integers".to_string())
        })
        .collect()
}

/// Apply one patch to the tree. Ops:
/// - `{"op":"add","path":[..,i],"node":{..}}`     insert node at index i
/// - `{"op":"remove","path":[..,i]}`              remove node at index i
/// - `{"op":"update","path":[..,i],"props":{..}}` shallow-merge props into node
pub fn apply_patch(tree: &mut Value, patch: &Value) -> Result<(), String> {
    let op = patch
        .get("op")
        .and_then(|v| v.as_str())
        .ok_or("patch lacks an \"op\"")?;
    let path = path_of(patch)?;
    match op {
        "add" => {
            let node = patch.get("node").ok_or("add patch lacks a \"node\"")?;
            validate_node(node)?; // vocabulary fence, checked BEFORE mutation
            let (children, idx) = resolve(tree, &path)?;
            if idx > children.len() {
                return Err(format!("add index {idx} out of range 0..={}", children.len()));
            }
            children.insert(idx, node.clone());
        }
        "remove" => {
            let (children, idx) = resolve(tree, &path)?;
            if idx >= children.len() {
                return Err(format!("remove index {idx} out of range"));
            }
            children.remove(idx);
        }
        "update" => {
            let props = patch
                .get("props")
                .and_then(|p| p.as_object())
                .ok_or("update patch lacks a \"props\" object")?;
            if props.contains_key("type") {
                return Err("update may not change a node's type (add+remove instead)".into());
            }
            let (children, idx) = resolve(tree, &path)?;
            let node = children
                .get_mut(idx)
                .ok_or_else(|| format!("update index {idx} out of range"))?;
            let obj = node.as_object_mut().ok_or("target node is not an object")?;
            for (k, v) in props {
                obj.insert(k.clone(), v.clone());
            }
        }
        other => return Err(format!("unknown patch op '{other}' (add/remove/update)")),
    }
    Ok(())
}
