//! tokens.rs — 樣式層的 capability sandbox(純邏輯,native 可測)。
//!
//! registry 與 styles/tokens.scss 一一對應(production 應由單一來源生成兩者;
//! PoC 以測試守住對應)。AI 生成的 style spec 只能:
//!   1. 使用被授權的樣式屬性(GRANTED)
//!   2. 引用 registry 內的 token 名
//! 其餘一律拒絕——與 DSL 的「fetch() 被 codegen 拒絕」同構,是樣式層的 import 表。

pub const COLORS: [&str; 9] = [
    "primary", "success", "warning", "danger",
    "surface-1", "surface-2", "surface-3", "text", "text-dim",
];
pub const SPACES: [&str; 7] = ["0", "1", "2", "3", "4", "5", "6"];
pub const RADII: [&str; 5] = ["0", "1", "2", "3", "full"];
pub const FONTS: [&str; 4] = ["1", "2", "3", "4"];

/// (spec 屬性名, CSS 屬性, token 命名空間, registry)
const GRANTED: [(&str, &str, &str, &[&str]); 6] = [
    ("color", "color", "color", &COLORS),
    ("background", "background-color", "color", &COLORS),
    ("padding", "padding", "space", &SPACES),
    ("gap", "gap", "space", &SPACES),
    ("radius", "border-radius", "radius", &RADII),
    ("font", "font-size", "font", &FONTS),
];

/// 驗證 style spec(JSON object)→ 合法則產出 `css-prop:var(--tk-ns-token);…`。
pub fn style_of(spec: &serde_json::Map<String, serde_json::Value>) -> Result<String, String> {
    let mut out = String::new();
    for (k, v) in spec {
        let val = v
            .as_str()
            .ok_or_else(|| format!("'{k}' value must be a token name (string)"))?;
        let (_, css, ns, reg) = GRANTED
            .iter()
            .find(|(p, ..)| p == k)
            .ok_or_else(|| {
                format!(
                    "unauthorized style property '{k}' — granted props: [{}]",
                    GRANTED.map(|g| g.0).join(", ")
                )
            })?;
        if !reg.contains(&val) {
            return Err(format!(
                "'{val}' is not a design token — granted {ns} tokens: [{}]",
                reg.join(", ")
            ));
        }
        out.push_str(&format!("{css}:var(--tk-{ns}-{val});"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map, Value};

    fn spec(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn valid_spec_emits_token_vars() {
        let s = super::style_of(&spec(json!(
            {"background": "surface-3", "color": "success", "padding": "4", "radius": "2"}
        )))
        .unwrap();
        assert!(s.contains("background-color:var(--tk-color-surface-3);"));
        assert!(s.contains("color:var(--tk-color-success);"));
        assert!(s.contains("padding:var(--tk-space-4);"));
        assert!(s.contains("border-radius:var(--tk-radius-2);"));
    }

    #[test]
    fn raw_css_value_rejected() {
        let e = super::style_of(&spec(json!({"color": "#ff0000"}))).unwrap_err();
        assert!(e.contains("不是 design token"), "{e}");
        assert!(e.contains("primary"), "應列出 granted tokens: {e}");
    }

    #[test]
    fn unknown_prop_rejected() {
        // position/z-index/content 等都不在授權清單——樣式攻擊面被鎖死
        let e = super::style_of(&spec(json!({"position": "fixed"}))).unwrap_err();
        assert!(e.contains("未授權的樣式屬性"), "{e}");
    }

    #[test]
    fn unknown_token_rejected() {
        let e = super::style_of(&spec(json!({"padding": "999"}))).unwrap_err();
        assert!(e.contains("不是 design token"), "{e}");
    }

    #[test]
    fn non_string_value_rejected() {
        assert!(super::style_of(&spec(json!({"padding": 4}))).is_err());
    }
}
