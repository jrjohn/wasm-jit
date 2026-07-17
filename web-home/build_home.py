# -*- coding: utf-8 -*-
"""Build the arcana.boo homepage (EN default + zh) — v2: v1's palette & layout
restored (theme-adaptive hero, fence back in the hero), the live wasm-jit
compiler panel becomes its own section §00 right after the hero, and the sky
is driven by TWO theme seeds (light/dark) — recompiled live when the theme
flips, so the light theme keeps v1's watercolor look."""
import html, os, re

TMP = os.path.expanduser("~/.claude/jobs/793080c4/tmp")
WT = os.path.expanduser("~/Documents/projects/ai/wasm-jit/.claude/worktrees/canvas-poc")
SEED_L = open(os.path.join(WT, "examples/homepage-light.dsl"), encoding="utf-8").read().strip()
SEED_D = open(os.path.join(WT, "examples/homepage-dark.dsl"), encoding="utf-8").read().strip()

zh = open(os.path.join(TMP, "wasm-jit-architecture.html"), encoding="utf-8").read()
en = open(os.path.join(TMP, "wasm-jit-architecture.en.html"), encoding="utf-8").read()

STYLE = zh[zh.index("<style>"): zh.index("</style>") + 8]

EXTRA_CSS = """
<style>
  /* live seed panel (§00) */
  .seedbox{background:var(--surface);border:1px solid var(--line);border-radius:14px;box-shadow:var(--shadow);overflow:hidden;max-width:860px;margin:34px 0 0 40px}
  @media (max-width:640px){.seedbox{margin-left:0}}
  .seedbox .fence-head{border-bottom:1px solid var(--line)}
  #seed{display:block;width:100%;height:212px;resize:vertical;border:0;outline:none;
    background:var(--sunk);color:var(--ink);font-family:var(--font-mono);font-size:12.5px;
    line-height:1.7;padding:14px 18px;box-sizing:border-box}
  .seed-bar{display:flex;flex-wrap:wrap;align-items:center;gap:10px;padding:12px 16px;
    border-top:1px solid var(--line);background:var(--surface-2)}
  .seed-bar button{font-family:var(--font-mono);font-size:12.5px;padding:7px 14px;border-radius:8px;
    border:1px solid color-mix(in oklab,var(--accent) 45%,var(--line));background:transparent;
    color:var(--accent);cursor:pointer}
  .seed-bar button:hover{background:color-mix(in oklab,var(--accent) 12%,transparent)}
  .seed-bar button.warn{border-color:color-mix(in oklab,var(--reject) 55%,var(--line));color:var(--reject)}
  .seed-bar button.warn:hover{background:color-mix(in oklab,var(--reject) 10%,transparent)}
  #seed-status{font-size:12px;line-height:1.5;flex:1;min-width:220px;font-family:var(--font-mono)}
  #seed-status.ok2{color:var(--pass)} #seed-status.no2{color:var(--reject)}
  .worldshot img{display:block;width:100%;max-width:860px;border-radius:13px;border:1px solid var(--line);box-shadow:var(--shadow)}
</style>
"""

def build(lang):
    page = en if lang == "en" else zh
    body = page[page.index("</style>") + 8: page.index("<script>")]

    if lang == "en":
        sec_title = "The sky you're under"
        sec_sub = "This page opens with its own proof"
        lede = ("Everything moving above — the moon and its halo, the drifting beings, the links that form and break, "
                "the gliding light, the snow — is <b>one WASM module</b>, compiled from the tiny script below "
                "<b>in your browser, just now</b>, by the wasm-jit compiler (itself Rust compiled to WASM). "
                "Its entire world is 10 drawing primitives. Edit it and recompile; or try to break out.")
        panel_head = '<span><b>THE SEED</b> · run(t, w, h) — recompiled live; it follows your light/dark theme (two seeds, one geometry)</span><span>examples/homepage-*.dsl</span>'
        btn_run, btn_violate, btn_restore = "Recompile ▸", "Try to break out — fetch()", "Restore"
        ok_tpl = "wasm-jit compiled {b} bytes in {m} ms — this module's entire world: 10 drawing primitives. fetch() does not exist here."
        err_tpl = "refused at compile time: {e}"
        shot_cap = "The seed above, manifested — <code>worlds/moon3-spring.json</code> loaded into the live world: the fisherman on the cold river, the water-blue moon, birds aloft and fish below. Every being is a sandboxed WASM cell (soul + skin); some carry a live mind."
        shot_alt = "The moon3-spring world: a fisherman on a river under a blue moon, birds and fish around"
        head_title = "wasm-jit — software with no fixed screen"
        head_desc = "Describe it and the interface appears — and the generated code can only touch what you allow. The sky on this page is a wasm-jit module, compiled live in your browser."
        lang_attr = "en"
        other_link = ('<a href="/index.zh.html" style="margin-left:auto;font-family:var(--font-mono);'
                      'font-size:12px;color:var(--ink-faint);border:0;letter-spacing:.05em">中文 ↗</a>')
        old_link = re.compile(r'<a href="architecture-anatomy\.html"[^>]*>中文 ↗</a>')
    else:
        sec_title = "頭頂這片天"
        sec_sub = "This page opens with its own proof"
        lede = ("上方所有會動的——月與月暈、漂移的眾生、時斷時續的連線、行走的光、飄雪——是<b>一顆 WASM 模組</b>,"
                "由下面這段小腳本<b>此刻在你的瀏覽器裡</b>編譯而成(編譯器本身是 Rust 編成的 WASM)。"
                "它的整個世界只有 10 個繪圖原語。改改看、重編看看;或者,試著越界。")
        panel_head = '<span><b>種子</b> · run(t, w, h) — 活編譯;隨你的亮/暗主題切換(兩顆種子,同一套幾何)</span><span>examples/homepage-*.dsl</span>'
        btn_run, btn_violate, btn_restore = "重新編譯 ▸", "試著越界 — fetch()", "還原"
        ok_tpl = "wasm-jit 編譯 {b} bytes,{m} ms — 這顆模組的整個世界:10 個繪圖原語。這裡沒有 fetch()。"
        err_tpl = "編譯期拒絕:{e}"
        shot_cap = "上面那顆種子,顯化之後 — <code>worlds/moon3-spring.json</code> 載入活世界:寒江上的漁翁、水藍的月、天上的鳥、水下的魚。每一位住民都是沙箱裡的 WASM 細胞(魂+皮);有些還載著一顆活的心。"
        shot_alt = "moon3-spring 世界:藍月下漁翁泛舟,鳥與魚環繞"
        head_title = "wasm-jit — 沒有固定畫面的軟體"
        head_desc = "描述它,介面就顯化——而生成的程式碼只碰得到你允許的。這頁的天空,是一顆 wasm-jit 模組,在你的瀏覽器裡當場編譯。"
        lang_attr = "zh-Hant"
        other_link = ('<a href="/" style="margin-left:auto;font-family:var(--font-mono);'
                      'font-size:12px;color:var(--ink-faint);border:0;letter-spacing:.05em">EN ↗</a>')
        old_link = re.compile(r'<a href="architecture-anatomy\.en\.html"[^>]*>EN ↗</a>')

    body = old_link.sub(other_link, body)

    section0 = f'''
<section id="s0">
  <div class="wrap">
    <div class="sec-head"><span class="sec-num">00</span><h2 class="sec-zh">{sec_title}</h2></div>
    <div class="sec-en">{sec_sub}</div>
    <p class="lede">{lede}</p>
    <div class="seedbox">
      <div class="fence-head">{panel_head}</div>
      <textarea id="seed" spellcheck="false" aria-label="wasm-jit seed source">{html.escape(SEED_L)}</textarea>
      <div class="seed-bar">
        <button id="btn-run">{btn_run}</button>
        <button id="btn-violate" class="warn">{btn_violate}</button>
        <button id="btn-restore">{btn_restore}</button>
        <span id="seed-status"></span>
      </div>
    </div>
  </div>
</section>'''

    body = body.replace("</header>", "</header>\n" + section0, 1)

    # §05: the world screenshot after the JSON figure
    s5 = body.index('id="s5"')
    fig_end = body.index("</figure>", s5) + len("</figure>")
    shot = f'''
    <figure class="indent worldshot">
      <img src="/home-assets/world-moon3-spring.jpg" alt="{shot_alt}" loading="lazy">
      <figcaption>{shot_cap}</figcaption>
    </figure>'''
    body = body[:fig_end] + shot + body[fig_end:]

    script = '''
<script type="module">
import init, { compile_draw_wasm } from '/pkg/wasm_jit.js';
const SEED_LIGHT = __SEED_L__;
const SEED_DARK = __SEED_D__;
const OK = (b,m)=>__OK__;
const ERR = e=>__ERR__;
(async function(){
  const c=document.getElementById('sky'); if(!c) return;
  const ctx=c.getContext('2d');
  const reduce = window.matchMedia && window.matchMedia('(prefers-reduced-motion:reduce)').matches;
  let W=0,H=0; const dpr=Math.min(window.devicePixelRatio||1,2);
  function size(){ const r=c.parentElement.getBoundingClientRect(); W=r.width; H=r.height;
    c.width=W*dpr; c.height=H*dpr; ctx.setTransform(dpr,0,0,dpr,0,0); }
  function theme(){ const a=document.documentElement.getAttribute('data-theme');
    if(a) return a;
    return (window.matchMedia && window.matchMedia('(prefers-color-scheme:dark)').matches)?'dark':'light'; }
  let drawA=a=>`hsla(200,62%,62%,${a})`;
  const env={ sin:Math.sin, cos:Math.cos,
    hue:v=>{const Hh=(((v%1)+1)%1)*360;const s=`hsl(${Hh},62%,62%)`;ctx.strokeStyle=s;ctx.fillStyle=s;drawA=a=>`hsla(${Hh},62%,62%,${a})`;},
    rgb:(r,g,b)=>{const R=Math.max(0,Math.min(255,r*255))|0,G=Math.max(0,Math.min(255,g*255))|0,B=Math.max(0,Math.min(255,b*255))|0;const s=`rgb(${R},${G},${B})`;ctx.strokeStyle=s;ctx.fillStyle=s;drawA=a=>`rgba(${R},${G},${B},${a})`;},
    hsl:(hh,ss,ll)=>{const Hh=(((hh%1)+1)%1)*360,S=Math.max(0,Math.min(1,ss))*100,L=Math.max(0,Math.min(1,ll))*100;const s=`hsl(${Hh},${S}%,${L}%)`;ctx.strokeStyle=s;ctx.fillStyle=s;drawA=a=>`hsla(${Hh},${S}%,${L}%,${a})`;},
    disc:(x,y,r)=>{ctx.beginPath();ctx.arc(x,y,Math.max(r,0),0,6.2832);ctx.fill();},
    ring:(x,y,r)=>{ctx.beginPath();ctx.arc(x,y,Math.max(r,0),0,6.2832);ctx.stroke();},
    arc:(x,y,r,a0,a1)=>{ctx.beginPath();ctx.arc(x,y,Math.max(r,0),a0,a1);ctx.stroke();},
    line:(x1,y1,x2,y2)=>{ctx.beginPath();ctx.moveTo(x1,y1);ctx.lineTo(x2,y2);ctx.stroke();},
    glow:(x,y,r)=>{r=Math.max(r,0); if(!r)return; const g=ctx.createRadialGradient(x,y,0,x,y,r);
      g.addColorStop(0,drawA(0.55)); g.addColorStop(1,drawA(0)); const k=ctx.fillStyle;
      ctx.fillStyle=g; ctx.beginPath(); ctx.arc(x,y,r,0,6.2832); ctx.fill(); ctx.fillStyle=k; } };
  const seedEl=document.getElementById('seed'), st=document.getElementById('seed-status');
  let curTheme=theme(), edited=false;
  if(seedEl) seedEl.value = curTheme==='dark' ? SEED_DARK : SEED_LIGHT;
  if(seedEl) seedEl.addEventListener('input',()=>{ edited=true; });
  let run=null;
  function compile(){
    try{
      const a=performance.now();
      const bytes=compile_draw_wasm(seedEl.value);
      const inst=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env});
      run=inst.exports.run;
      if(st){ st.textContent=OK(bytes.length,(performance.now()-a).toFixed(1)); st.className='ok2'; }
    }catch(e){ if(st){ st.textContent=ERR(e && e.message ? e.message : String(e)); st.className='no2'; } }
  }
  function onTheme(){ const th=theme(); if(th===curTheme) return; curTheme=th;
    if(!edited && seedEl){ seedEl.value = th==='dark' ? SEED_DARK : SEED_LIGHT; compile(); } if(reduce) frame(); }
  await init(); size(); compile();
  const t0=performance.now();
  function frame(){ if(run){ ctx.clearRect(0,0,W,H); ctx.lineWidth=1.15; ctx.lineCap='round';
    try{ run((performance.now()-t0)/1000+2.0, W, H); }catch(e){ run=null; } } }
  function loop(){ frame(); if(!reduce) requestAnimationFrame(loop); }
  if(reduce){ frame(); } else requestAnimationFrame(loop);
  window.addEventListener('resize',()=>{ size(); if(reduce) frame(); });
  new MutationObserver(onTheme).observe(document.documentElement,{attributes:true,attributeFilter:['data-theme']});
  if(window.matchMedia) window.matchMedia('(prefers-color-scheme:dark)').addEventListener('change',onTheme);
  const $=id=>document.getElementById(id);
  if($('btn-run')) $('btn-run').onclick=()=>compile();
  if($('btn-violate')) $('btn-violate').onclick=()=>{ edited=true; seedEl.value='// a seed that reaches for the net\\nfetch(t);\\n0.0'; compile(); };
  if($('btn-restore')) $('btn-restore').onclick=()=>{ edited=false; seedEl.value = curTheme==='dark' ? SEED_DARK : SEED_LIGHT; compile(); };
  if(!reduce && 'IntersectionObserver' in window){
    document.querySelectorAll('section .wrap > *').forEach(el=>el.classList.add('reveal'));
    const io=new IntersectionObserver(es=>{es.forEach(e=>{if(e.isIntersecting){e.target.classList.add('in');io.unobserve(e.target);}});},{threshold:0.08,rootMargin:'0px 0px -8% 0px'});
    document.querySelectorAll('.reveal').forEach(el=>io.observe(el));
  }
})();
</script>'''
    import json as _json
    ok_js = "`" + ok_tpl.replace("{b}", "${b}").replace("{m}", "${m}") + "`"
    err_js = "`" + err_tpl.replace("{e}", "${e}") + "`"
    script = (script.replace("__SEED_L__", _json.dumps(SEED_L))
                    .replace("__SEED_D__", _json.dumps(SEED_D))
                    .replace("__OK__", ok_js).replace("__ERR__", err_js))

    doc = f'''<!doctype html>
<html lang="{lang_attr}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{head_title}</title>
<meta name="description" content="{head_desc}">
<meta property="og:title" content="{head_title}">
<meta property="og:description" content="{head_desc}">
<meta property="og:type" content="website">
<meta property="og:url" content="https://arcana.boo/">
<meta property="og:image" content="https://arcana.boo/home-assets/world-moon3-spring.jpg">
<meta name="twitter:card" content="summary_large_image">
{STYLE}
{EXTRA_CSS}
</head>
<body>
{body}
{script}
</body>
</html>'''
    out = os.path.join(TMP, "stage", "index.html" if lang == "en" else "index.zh.html")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    open(out, "w", encoding="utf-8").write(doc)
    print(f"{lang}: {len(doc.encode('utf-8'))} bytes -> {out}")

build("en")
build("zh")
