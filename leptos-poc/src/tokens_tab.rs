//! tokens_tab.rs — PoC 4:design-token 化的樣式層。
//! 上半:token rails(SCSS 生成的變數,體)以 swatch 呈現。
//! 下半:模擬 AI 生成的 style spec(JSON)——只能引用 token;
//!       raw CSS / 未授權屬性在驗證層被拒(樣式的 capability sandbox)。

use crate::tokens::{style_of, COLORS};
use leptos::prelude::*;

const DEFAULT_SPEC: &str =
    r#"{"background":"surface-3","color":"success","padding":"5","radius":"2","font":"3"}"#;

#[component]
pub fn TokensPoc() -> impl IntoView {
    let spec_text = RwSignal::new(DEFAULT_SPEC.to_string());
    let style_str = RwSignal::new(String::new());
    let err = RwSignal::new(String::new());

    let run = move |src: &str| match serde_json::from_str::<
        serde_json::Map<String, serde_json::Value>,
    >(src)
    {
        Ok(spec) => match style_of(&spec) {
            Ok(s) => {
                style_str.set(s);
                err.set(String::new());
            }
            Err(e) => err.set(e),
        },
        Err(e) => err.set(format!("JSON 解析失敗:{e}")),
    };

    run(DEFAULT_SPEC); // 初始顯化一次

    let apply = move |_| run(&spec_text.get());
    let violate = move |_| {
        let bad = r##"{"color":"#ff0000","position":"fixed"}"##;
        spec_text.set(bad.to_string());
        run(bad);
    };

    view! {
        <p class="sub">
            "樣式的體用:SCSS 在編譯期生成 token rails(體);AI 的顯化只能引用 token 組合(用)。"
            "raw CSS 值與未授權屬性在驗證層被拒——與 DSL 的 fetch() 拒絕同構,樣式層的 import 表。"
        </p>

        <h2>"Token rails(styles/tokens.scss 生成,不可由顯化修改)"</h2>
        <div class="swatches">
            {COLORS
                .iter()
                .map(|c| {
                    view! {
                        <div class="swatch" style=format!("background:var(--tk-color-{c})")>
                            <span>{*c}</span>
                        </div>
                    }
                })
                .collect_view()}
        </div>

        <h2>"顯化(模擬 AI 生成的 style spec — 只准引用 token)"</h2>
        <textarea class="tok-spec" rows="3"
            prop:value=move || spec_text.get()
            on:input=move |ev| spec_text.set(event_target_value(&ev))></textarea>
        <div class="tok-row">
            <button class="apply tok-apply" on:click=apply>"Apply spec"</button>
            <button class="tok-violate" on:click=violate>"試圖越權(raw CSS + position)"</button>
        </div>
        <Show when=move || !err.get().is_empty()>
            <div class="tok-err">{move || err.get()}</div>
        </Show>

        <div class="tok-preview" style=move || style_str.get()>
            "預覽卡片 — 我身上的每一筆樣式都是 var(--tk-*),沒有一個 raw 值。"
            <br />
            <code style="font-size:var(--tk-font-1);opacity:.75">
                {move || style_str.get()}
            </code>
        </div>
    }
}
