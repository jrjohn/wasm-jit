# -*- coding: utf-8 -*-
"""Build the arcana.boo homepage (EN default + zh) from the anatomy pages:
- hero gains a LIVE wasm-jit compiler panel (editable seed, recompile, violate)
- the static fence figure moves into §02
- §05 gains the moon3-spring world screenshot
- full standalone docs with head/OG; module script drives the sky from the seed
"""
import html, os, re

TMP = os.path.expanduser("~/.claude/jobs/793080c4/tmp")
WT = os.path.expanduser("~/Documents/projects/ai/wasm-jit/.claude/worktrees/canvas-poc")
SEED = open(os.path.join(WT, "examples/homepage.dsl"), encoding="utf-8").read().strip()

def load(p):
    return open(p, encoding="utf-8").read()

zh = load(os.path.join(TMP, "wasm-jit-architecture.html"))
en = load(os.path.join(TMP, "wasm-jit-architecture.en.html"))

STYLE = zh[zh.index("<style>"): zh.index("</style>") + 8]

EXTRA_CSS = """
<style>
  /* hero commits to one world: the ink night (both themes) */
  .hero{--bg:#0b111a;--surface:#111a26;--surface-2:#0e1621;--sunk:#0a0f17;
    --ink:#dde6f0;--ink-soft:#9db0c4;--ink-faint:#63748a;--line:#213042;
    --accent:#5fb0d4;--accent-bright:#82caea;--moon:#7cc6e6;--ember:#d29a68;
    --pass:#5fbb8c;--reject:#d3817e;background:#0b111a;color:var(--ink);color-scheme:dark}
  /* live seed panel */
  .seed-wrap{padding:0}
  #seed{display:block;width:100%;height:216px;resize:vertical;border:0;outline:none;
    background:var(--sunk);color:var(--ink);font-family:var(--font-mono);font-size:12.5px;
    line-height:1.7;padding:14px 18px;box-sizing:border-box}
  #seed:focus{background:#0d1420}
  .seed-bar{display:flex;flex-wrap:wrap;align-items:center;gap:10px;padding:12px 16px;
    border-top:1px solid var(--line);background:var(--surface-2)}
  .seed-bar button{font-family:var(--font-mono);font-size:12.5px;padding:7px 14px;border-radius:8px;
    border:1px solid color-mix(in oklab,var(--accent) 45%,var(--line));background:transparent;
    color:var(--accent);cursor:pointer}
  .seed-bar button:hover{background:color-mix(in oklab,var(--accent) 12%,transparent)}
  .seed-bar button.warn{border-color:color-mix(in oklab,var(--reject) 55%,var(--line));color:var(--reject)}
  .seed-bar button.warn:hover{background:color-mix(in oklab,var(--reject) 10%,transparent)}
  #seed-status{font-size:12px;line-height:1.5;flex:1;min-width:220px}
  #seed-status.ok2{color:var(--pass)} #seed-status.no2{color:var(--reject)}
  .worldshot img{display:block;width:100%;max-width:860px;border-radius:13px;border:1px solid var(--line);box-shadow:var(--shadow)}
</style>
"""

def body_of(page):
    b = page[page.index("</style>") + 8: page.index("<script>")]
    return b

def carve_fence(body):
    a = body.index('<div class="fence">')
    # find matching close: fence block ends before the closing of hero .wrap: '    </div>\n  </div>\n</header>'
    z = body.index("</header>")
    seg = body[a:z]
    # fence block is the last element in .wrap; strip trailing wrap/header closers
    fence = seg[: seg.rindex("</div>\n  </div>\n")] + "</div>"
    rest = body[:a] + body[a + len(seg):]  # header now missing its closers
    # re-add the wrap+hero closers before </header>
    rest = rest.replace("</header>", "  </div>\n</header>", 1)
    return fence, rest

def build(lang):
    page = en if lang == "en" else zh
    body = body_of(page)
    fence, body = carve_fence(body)

    if lang == "en":
        panel_head = '<span><b>THE SEED</b> — everything moving above is this script, compiled to WASM in your browser right now. Edit it.</span><span>run(t, w, h)</span>'
        btn_run, btn_violate, btn_restore = "Recompile ▸", "Try to break out — fetch()", "Restore"
        ok_tpl = "wasm-jit compiled {b} bytes in {m} ms — this module's entire world: 10 drawing primitives. fetch() does not exist here."
        err_tpl = "refused at compile time: {e}"
        shot_cap = "The seed above, manifested — <code>worlds/moon3-spring.json</code> loaded into the live world: the fisherman on the cold river, the water-blue moon, birds aloft and fish below. Every being is a sandboxed WASM cell (soul + skin); some carry a live mind."
        shot_alt = "The moon3-spring world: a fisherman on a river under a blue moon, birds and fish around"
        head_title = "wasm-jit — software with no fixed screen"
        head_desc = "Describe it and the interface appears — and the generated code can only touch what you allow. The night sky on this page is a 2KB WASM module, compiled live in your browser by wasm-jit."
        lang_attr = "en"
        other_link = ('<a href="/index.zh.html" style="margin-left:auto;font-family:var(--font-mono);'
                      'font-size:12px;color:var(--ink-faint);border:0;letter-spacing:.05em">中文 ↗</a>')
        old_link = re.compile(r'<a href="architecture-anatomy\.html"[^>]*>中文 ↗</a>')
    else:
        panel_head = '<span><b>種子</b> — 上方所有會動的,都是這段腳本,此刻在你的瀏覽器裡編成 WASM。可以改。</span><span>run(t, w, h)</span>'
        btn_run, btn_violate, btn_restore = "重新編譯 ▸", "試著越界 — fetch()", "還原"
        ok_tpl = "wasm-jit 編譯 {b} bytes,{m} ms — 這顆模組的整個世界:10 個繪圖原語。這裡沒有 fetch()。"
        err_tpl = "編譯期拒絕:{e}"
        shot_cap = "上面那顆種子,顯化之後 — <code>worlds/moon3-spring.json</code> 載入活世界:寒江上的漁翁、水藍的月、天上的鳥、水下的魚。每一位住民都是沙箱裡的 WASM 細胞(魂+皮);有些還載著一顆活的心。"
        shot_alt = "moon3-spring 世界:藍月下漁翁泛舟,鳥與魚環繞"
        head_title = "wasm-jit — 沒有固定畫面的軟體"
        head_desc = "描述它,介面就顯化——而生成的程式碼只碰得到你允許的。這頁的夜空,是一顆 2KB 的 WASM 模組,由 wasm-jit 在你的瀏覽器裡當場編譯。"
        lang_attr = "zh-Hant"
        other_link = ('<a href="/" style="margin-left:auto;font-family:var(--font-mono);'
                      'font-size:12px;color:var(--ink-faint);border:0;letter-spacing:.05em">EN ↗</a>')
        old_link = re.compile(r'<a href="architecture-anatomy\.en\.html"[^>]*>EN ↗</a>')

    body = old_link.sub(other_link, body)

    panel = f'''
    <div class="fence" id="seedpanel">
      <div class="fence-head">{panel_head}</div>
      <div class="seed-wrap">
        <textarea id="seed" spellcheck="false" aria-label="wasm-jit seed source">{html.escape(SEED)}</textarea>
        <div class="seed-bar">
          <button id="btn-run">{btn_run}</button>
          <button id="btn-violate" class="warn">{btn_violate}</button>
          <button id="btn-restore">{btn_restore}</button>
          <span id="seed-status" class="mono"></span>
        </div>
      </div>
    </div>'''

    # live panel goes where the fence used to end (last child of hero .wrap)
    body = body.replace("  </div>\n</header>", panel + "\n  </div>\n</header>", 1)

    # fence figure moves into §02, after its first measure paragraph
    s2_anchor = body.index("</div>", body.index('id="s2"'))
    # place after the first ".body measure" block's close in s2: find '</p>\n    </div>' after s2 start
    m = body.index("</p>\n    </div>", body.index('id="s2"'))
    ins = m + len("</p>\n    </div>")
    body = body[:ins] + "\n    <figure class=\"indent\">" + fence + "</figure>" + body[ins:]

    # §05: insert the world screenshot after the JSON figure
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
const OK = (b,m)=>__OK__;
const ERR = e=>__ERR__;
(async function(){
  const c=document.getElementById('sky'); if(!c) return;
  const ctx=c.getContext('2d');
  const reduce = window.matchMedia && window.matchMedia('(prefers-reduced-motion:reduce)').matches;
  let W=0,H=0; const dpr=Math.min(window.devicePixelRatio||1,2);
  function size(){ const r=c.parentElement.getBoundingClientRect(); W=r.width; H=r.height;
    c.width=W*dpr; c.height=H*dpr; ctx.setTransform(dpr,0,0,dpr,0,0); }
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
  const SEED0=seedEl?seedEl.value:'';
  let run=null;
  function compile(){
    try{
      const a=performance.now();
      const bytes=compile_draw_wasm(seedEl.value);
      const inst=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env});
      run=inst.exports.run;
      if(st){ st.textContent=OK(bytes.length,(performance.now()-a).toFixed(1)); st.className='mono ok2'; }
    }catch(e){ if(st){ st.textContent=ERR(e && e.message ? e.message : String(e)); st.className='mono no2'; } }
  }
  await init(); size(); compile();
  const t0=performance.now();
  function frame(){ if(run){ ctx.clearRect(0,0,W,H); ctx.lineWidth=1.3; ctx.lineCap='round';
    try{ run((performance.now()-t0)/1000+2.0, W, H); }catch(e){ run=null; } } }
  function loop(){ frame(); if(!reduce) requestAnimationFrame(loop); }
  if(reduce){ frame(); } else requestAnimationFrame(loop);
  window.addEventListener('resize',()=>{ size(); if(reduce) frame(); });
  const $=id=>document.getElementById(id);
  if($('btn-run')) $('btn-run').onclick=()=>compile();
  if($('btn-violate')) $('btn-violate').onclick=()=>{ seedEl.value='// a seed that reaches for the net\\nfetch(t);\\n0.0'; compile(); };
  if($('btn-restore')) $('btn-restore').onclick=()=>{ seedEl.value=SEED0; compile(); };
  if(!reduce && 'IntersectionObserver' in window){
    document.querySelectorAll('section .wrap > *').forEach(el=>el.classList.add('reveal'));
    const io=new IntersectionObserver(es=>{es.forEach(e=>{if(e.isIntersecting){e.target.classList.add('in');io.unobserve(e.target);}});},{threshold:0.08,rootMargin:'0px 0px -8% 0px'});
    document.querySelectorAll('.reveal').forEach(el=>io.observe(el));
  }
})();
</script>'''
    ok_js = "`" + ok_tpl.replace("{b}", "${b}").replace("{m}", "${m}") + "`"
    err_js = "`" + err_tpl.replace("{e}", "${e}") + "`"
    script = script.replace("__OK__", ok_js).replace("__ERR__", err_js)

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
    print(f"{lang}: {len(doc)} bytes -> {out}")

build("en")
build("zh")
