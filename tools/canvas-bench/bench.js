'use strict';
// Стенд сравнения рендер-путей для анимации «панель поверх размытого фона».
// Два набора вариантов (runBench(cycles, set)):
//   'heavy' (дефолт) — DOM-сцена со сложностью лаунчера:
//     hv-full        — панель min(940,95vw), backdrop live всю анимацию,
//                      element-filter blur(14px) в keyframes, V22 гасит
//                      7 внешних блюров, язычок новостей едет news-away
//                      (режим full — дефолт лаунчера)
//     hv-baked       — идея №1: запечённый фон-блюр вместо live backdrop
//     hv-smooth      — backdrop панели/навигации выключен на время rise/sink
//     hv-noblur      — контроль без backdrop и без element-filter (пол)
//     hv-full-nov22  — full без V22 (внешние блюры живут, «как до фикса»)
//   'light' — исходный набор, рендер-пути на простой сцене:
//     dom-blur / dom-noblur / c2d-live / c2d-baked / webgl-live
// Замер: N циклов открыть/закрыть, сбор длин кадров rAF — методика
// tools/perf-anim-ab.js (фазы open-anim/close-anim, пороги 33/50/100мс).

//── общие размеры сцены ──
const VW = innerWidth, VH = innerHeight;
const DPR = Math.min(window.devicePixelRatio || 1, 1.5);
const PW = Math.min(720, VW - 160);        // финальный размер панели, CSS px
const PH = Math.min(460, VH - 140);
const CX = VW / 2, CY = VH / 2;
const BLUR = 24;                           // CSS px — как дефолт лаунчера
const PADF = 72;                           // запас региона блюра (3σ)
const DUR = 400;                           // длительность анимации, мс
const TINT = [16 / 255, 18 / 255, 26 / 255, 0.72]; // rgba панели
const REGION_W = Math.round((PW + 2 * PADF) * DPR);
const REGION_H = Math.round((PH + 2 * PADF) * DPR);
const HALF_W = Math.ceil(REGION_W / 2);
const HALF_H = Math.ceil(REGION_H / 2);

const easeOut = t => 1 - Math.pow(1 - t, 3);
const sleep = ms => new Promise(r => setTimeout(r, ms));
const now = () => performance.now();
const hud = document.getElementById('hud');

function rr(x, X, Y, W, H, r) {             // rounded-rect path
  x.beginPath();
  x.moveTo(X + r, Y);
  x.arcTo(X + W, Y, X + W, Y + H, r);
  x.arcTo(X + W, Y + H, X, Y + H, r);
  x.arcTo(X, Y + H, X, Y, r);
  x.arcTo(X, Y, X + W, Y, r);
  x.closePath();
}

//── обои (общие для всех вариантов) ──
const IMG = new Image();
const IMGREADY = new Promise((res, rej) => {
  IMG.onload = () => res();
  IMG.onerror = () => rej(new Error('обои не загрузились'));
});
IMG.src = document.getElementById('bgImg').src; // в standalone это data:URL

//── контент панели, запечённый в canvas (для canvas/webgl-вариантов) ──
function bakeContent() {
  const c = document.createElement('canvas');
  c.width = Math.round(PW * DPR);
  c.height = Math.round(PH * DPR);
  const x = c.getContext('2d');
  x.scale(DPR, DPR);
  x.fillStyle = '#eef1f7';
  x.font = '600 20px "Segoe UI",system-ui,sans-serif';
  x.fillText('Настройки', 22, 40);
  x.font = '400 15px "Segoe UI",system-ui,sans-serif';
  const rows = [
    ['Обои лаунчера', true],
    ['Проверять обновления', true],
    ['Свёрнутый режим', false],
    ['Звук при запуске', true],
  ];
  let y = 66;                              // как у DOM: head ≈57 + margin 8
  for (const [label, on] of rows) {
    x.fillStyle = 'rgba(255,255,255,0.05)';
    rr(x, 18, y, PW - 36, 49, 10);
    x.fill();
    x.fillStyle = '#c9d2e3';
    x.fillText(label, 34, y + 30);
    const sw = 44, sh = 22, sx = PW - 78, sy = y + (49 - sh) / 2;
    x.fillStyle = on ? '#3b82f6' : '#5b6474';
    rr(x, sx, sy, sw, sh, 11);
    x.fill();
    x.fillStyle = '#fff';
    x.beginPath();
    x.arc(on ? sx + sw - 11 : sx + 11, sy + sh / 2, 7, 0, 6.2832);
    x.fill();
    y += 65;
  }
  return c;
}

//── DOM-вариант (два режима: с блюром и контрольный без) ──
const domScene = document.getElementById('domScene');
const domPanel = document.getElementById('domPanel');

function domAnimate(dir) {                  // dir: 1 = открыть, 0 = закрыть
  domPanel.getAnimations().forEach(a => a.cancel());
  const s = dir ? 1.08 : 1, e = dir ? 1 : 1.08;
  const o0 = dir ? 0 : 1, o1 = dir ? 1 : 0;
  domPanel.animate([
    { transform: `translate(-50%,-50%) scale(${s})`, opacity: o0 },
    { transform: `translate(-50%,-50%) scale(${e})`, opacity: o1 },
  ], { duration: DUR, easing: 'cubic-bezier(0.33,1,0.68,1)', fill: 'forwards' });
}

const domVariant = (name, nb) => ({
  name,
  renderer: nb ? 'DOM (контроль, без блюра)' : 'DOM + backdrop-filter',
  async setup() {
    domPanel.style.width = PW + 'px';
    domPanel.style.height = PH + 'px';
  },
  show() {
    domScene.classList.add('on');
    domScene.classList.toggle('nb', nb);
  },
  hide() {
    domPanel.getAnimations().forEach(a => a.cancel());
    domScene.classList.remove('on');
    domScene.classList.remove('nb');
  },
  open() { domAnimate(1); },
  close() { domAnimate(0); },
});

//── canvas 2D: live (ctx.filter каждый кадр) и baked (blur запечён) ──
class C2D {
  constructor(canvasId, bakedMode) {
    this.cv = document.getElementById(canvasId);
    this.bakedMode = bakedMode;
    this.name = bakedMode ? 'c2d-baked' : 'c2d-live';
    this.renderer = 'Canvas 2D (Skia' + (bakedMode ? ', blur запечён)' : ', ctx.filter)');
    this.vis = 0;
    this.anim = null;
    this.raf = 0;
  }
  async setup() {
    const W = Math.round(VW * DPR), H = Math.round(VH * DPR);
    this.cv.width = W;
    this.cv.height = H;
    this.ctx = this.cv.getContext('2d');
    // база: обои cover — рисуется один раз, каждый кадр только blit
    this.base = document.createElement('canvas');
    this.base.width = W;
    this.base.height = H;
    const bx = this.base.getContext('2d');
    const s = Math.max(W / IMG.width, H / IMG.height);
    const iw = IMG.width * s, ih = IMG.height * s;
    bx.drawImage(IMG, (W - iw) / 2, (H - ih) / 2, iw, ih);
    this.content = bakeContent();
    if (this.bakedMode) {
      const b = document.createElement('canvas');
      b.width = W;
      b.height = H;
      const bctx = b.getContext('2d');
      bctx.filter = `blur(${BLUR * DPR}px) saturate(1.2)`;
      bctx.drawImage(this.base, 0, 0);
      bctx.filter = 'none';
      this.baked = b;
    }
    this.vis = 0;
    this.stop();
  }
  show() { this.cv.classList.add('on'); this.draw(); }
  hide() { this.cv.classList.remove('on'); this.stop(); this.vis = 0; }
  open() { this.startAnim(1); }
  close() { this.startAnim(0); }
  startAnim(dir) {
    this.stop();
    this.anim = { from: this.vis, to: dir ? 1 : 0, t0: now() };
    const step = () => {
      if (!this.anim) { this.raf = 0; return; }
      const t = Math.min(1, (now() - this.anim.t0) / DUR);
      this.vis = this.anim.from + (this.anim.to - this.anim.from) * easeOut(t);
      this.draw();
      if (t >= 1) { this.anim = null; this.raf = 0; }
      else this.raf = requestAnimationFrame(step);
    };
    this.raf = requestAnimationFrame(step);
  }
  stop() {
    if (this.raf) cancelAnimationFrame(this.raf);
    this.raf = 0;
    this.anim = null;
  }
  draw() {
    const x = this.ctx, W = this.cv.width, H = this.cv.height;
    x.setTransform(1, 0, 0, 1, 0, 0);
    x.globalAlpha = 1;
    x.filter = 'none';
    x.clearRect(0, 0, W, H);
    x.drawImage(this.base, 0, 0);
    if (this.vis <= 0.002) return;
    const sc = 1 + 0.08 * (1 - this.vis);  // 1.08 → 1
    const pw = PW * DPR * sc, ph = PH * DPR * sc;
    const px = CX * DPR - pw / 2, py = CY * DPR - ph / 2;
    const r = 14 * DPR * sc;
    x.save();
    x.globalAlpha = this.vis;
    rr(x, px, py, pw, ph, r);
    x.clip();
    if (!this.bakedMode) {
      // blur фона под панелью — каждый кадр (аналог backdrop-filter)
      const m = PADF * DPR;                 // 3σ, чтобы края не темнели
      x.filter = `blur(${BLUR * DPR}px) saturate(1.2)`;
      x.drawImage(this.base,
        px - m, py - m, pw + 2 * m, ph + 2 * m,
        px - m, py - m, pw + 2 * m, ph + 2 * m);
      x.filter = 'none';
    } else {
      x.drawImage(this.baked, 0, 0);
    }
    x.fillStyle = `rgba(16,18,26,${TINT[3]})`;
    x.fillRect(px, py, pw, ph);
    x.drawImage(this.content, 0, 0, this.content.width, this.content.height,
      px, py, pw, ph);
    x.restore();
    x.save();
    x.globalAlpha = this.vis;
    x.strokeStyle = 'rgba(255,255,255,0.08)';
    x.lineWidth = DPR;
    rr(x, px + DPR / 2, py + DPR / 2, pw - DPR, ph - DPR, r);
    x.stroke();
    x.restore();
  }
}
//── WebGL: blur шейдером каждый кадр (half-res, 2 прохода) + composite ──
const VS_BG = `
attribute vec2 aPos;
uniform vec2 uRes, uOff, uSz;
varying vec2 vUV;
void main(){
  vec2 px = vec2((aPos.x*0.5+0.5)*uRes.x, (0.5-aPos.y*0.5)*uRes.y);
  vUV = (px - uOff) / uSz;
  gl_Position = vec4(aPos, 0.0, 1.0);
}`;
const FS_BG = `
precision mediump float;
uniform sampler2D uTex;
varying vec2 vUV;
void main(){ gl_FragColor = vec4(texture2D(uTex, vUV).rgb, 1.0); }`;

const VS_BLUR = `
attribute vec2 aPos;
uniform vec4 uOriginSize;   // xy = origin, zw = size (uv входной текстуры)
varying vec2 vUV;
void main(){
  vec2 t = aPos * 0.5 + 0.5;
  vUV = uOriginSize.xy + t * uOriginSize.zw;
  gl_Position = vec4(aPos, 0.0, 1.0);
}`;
const FS_BLUR = `
precision mediump float;
uniform sampler2D uTex;
uniform vec2 uStep;
varying vec2 vUV;
const float W0 = 0.227027;
const float W1 = 0.1945946;
const float W2 = 0.1216216;
const float W3 = 0.054054;
const float W4 = 0.016216;
void main(){
  vec4 c = texture2D(uTex, vUV) * W0;
  c += (texture2D(uTex, vUV + uStep)      + texture2D(uTex, vUV - uStep))      * W1;
  c += (texture2D(uTex, vUV + uStep*2.0)  + texture2D(uTex, vUV - uStep*2.0))  * W2;
  c += (texture2D(uTex, vUV + uStep*3.0)  + texture2D(uTex, vUV - uStep*3.0))  * W3;
  c += (texture2D(uTex, vUV + uStep*4.0)  + texture2D(uTex, vUV - uStep*4.0))  * W4;
  gl_FragColor = c;
}`;

const VS_PANEL = `
attribute vec2 aPos;
uniform vec4 uRect;         // x,y = левый-верх (device px), z,w = размер
uniform vec2 uRes;
varying vec2 vPx;
void main(){
  vec2 t = vec2(aPos.x*0.5+0.5, 0.5-aPos.y*0.5);
  vPx = uRect.xy + t * uRect.zw;
  vec2 ndc = vec2(vPx.x/uRes.x*2.0 - 1.0, 1.0 - vPx.y/uRes.y*2.0);
  gl_Position = vec4(ndc, 0.0, 1.0);
}`;
const FS_PANEL = `
precision highp float;
uniform sampler2D uBlur, uContent;
uniform vec4 uRegion, uRect;
uniform float uRadius, uOpacity;
uniform vec4 uTint;
varying vec2 vPx;
void main(){
  vec2 q = vPx - uRect.xy;
  vec2 hf = uRect.zw * 0.5;
  vec2 p = q - hf;
  vec2 qr = abs(p) - hf + uRadius;
  float d = length(max(qr, 0.0)) + min(max(qr.x, qr.y), 0.0) - uRadius;
  float mask = 1.0 - smoothstep(-0.5, 0.5, d);
  if (mask <= 0.001) discard;
  vec2 buv = (vPx - uRegion.xy) / uRegion.zw;
  vec3 blur = texture2D(uBlur, buv).rgb;
  vec3 outc = mix(blur, uTint.rgb, uTint.a);
  vec4 ct = texture2D(uContent, q / uRect.zw);
  outc = mix(outc, ct.rgb, ct.a);
  gl_FragColor = vec4(outc, mask * uOpacity);
}`;

class WGL {
  constructor() {
    this.name = 'webgl-live';
    this.renderer = 'WebGL';
    this.cv = document.getElementById('cvg');
    this.vis = 0;
    this.anim = null;
    this.raf = 0;
  }
  compile(vsSrc, fsSrc) {
    const gl = this.gl;
    const mk = (type, src) => {
      const s = gl.createShader(type);
      gl.shaderSource(s, src);
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS))
        throw new Error('shader: ' + gl.getShaderInfoLog(s));
      return s;
    };
    const p = gl.createProgram();
    gl.attachShader(p, mk(gl.VERTEX_SHADER, vsSrc));
    gl.attachShader(p, mk(gl.FRAGMENT_SHADER, fsSrc));
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS))
      throw new Error('link: ' + gl.getProgramInfoLog(p));
    const u = {};
    const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
    for (let i = 0; i < n; i++) {
      const info = gl.getActiveUniform(p, i);
      u[info.name] = gl.getUniformLocation(p, info.name);
    }
    return { p, u, aPos: gl.getAttribLocation(p, 'aPos') };
  }
  texFrom(src, w, h) {
    const gl = this.gl;
    const t = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, t);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, src);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    return t;
  }
  fbo(w, h) {
    const gl = this.gl;
    const t = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, t);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    const f = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, f);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, t, 0);
    if (gl.checkFramebufferStatus(gl.FRAMEBUFFER) !== gl.FRAMEBUFFER_COMPLETE)
      throw new Error('FBO неполон');
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    return { t, f };
  }
  async setup() {
    const W = Math.round(VW * DPR), H = Math.round(VH * DPR);
    this.cv.width = W;
    this.cv.height = H;
    const gl = this.gl = this.cv.getContext('webgl',
      { antialias: false, alpha: false, depth: false, stencil: false });
    if (!gl) throw new Error('WebGL недоступен');
    this.renderer = String(gl.getParameter(gl.RENDERER)) + ' / ' +
      String(gl.getParameter(gl.VENDOR));
    this.progBg = this.compile(VS_BG, FS_BG);
    this.progBlur = this.compile(VS_BLUR, FS_BLUR);
    this.progPanel = this.compile(VS_PANEL, FS_PANEL);
    // юнит-квад (triangle strip)
    this.quad = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.quad);
    gl.bufferData(gl.ARRAY_BUFFER,
      new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    // текстуры: обои и запечённый контент
    this.bgTex = this.texFrom(IMG, IMG.width, IMG.height);
    this.contentTex = this.texFrom(this.contentCv ||
      (this.contentCv = bakeContent()), Math.round(PW * DPR), Math.round(PH * DPR));
    // FBO полурегиона для двухпроходного блюра
    this.fboA = this.fbo(HALF_W, HALF_H);
    this.fboB = this.fbo(HALF_W, HALF_H);
    // cover-геометрия обоев в device px
    const s = Math.max(W / IMG.width, H / IMG.height);
    this.imgSz = [IMG.width * s, IMG.height * s];
    this.imgOff = [(W - this.imgSz[0]) / 2, (H - this.imgSz[1]) / 2];
    this.vis = 0;
    this.stop();
  }
  show() { this.cv.classList.add('on'); this.draw(); }
  hide() { this.cv.classList.remove('on'); this.stop(); this.vis = 0; }
  open() { this.startAnim(1); }
  close() { this.startAnim(0); }
  startAnim(dir) {
    this.stop();
    this.anim = { from: this.vis, to: dir ? 1 : 0, t0: now() };
    const step = () => {
      if (!this.anim) { this.raf = 0; return; }
      const t = Math.min(1, (now() - this.anim.t0) / DUR);
      this.vis = this.anim.from + (this.anim.to - this.anim.from) * easeOut(t);
      this.draw();
      if (t >= 1) { this.anim = null; this.raf = 0; }
      else this.raf = requestAnimationFrame(step);
    };
    this.raf = requestAnimationFrame(step);
  }
  stop() {
    if (this.raf) cancelAnimationFrame(this.raf);
    this.raf = 0;
    this.anim = null;
  }
  draw() {
    const gl = this.gl, W = this.cv.width, H = this.cv.height;
    gl.viewport(0, 0, W, H);
    gl.disable(gl.BLEND);
    gl.clearColor(0, 0, 0, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.quad);
    // фон (cover)
    let pr = this.progBg;
    gl.useProgram(pr.p);
    gl.enableVertexAttribArray(pr.aPos);
    gl.vertexAttribPointer(pr.aPos, 2, gl.FLOAT, false, 0, 0);
    gl.uniform2f(pr.u.uRes, W, H);
    gl.uniform2f(pr.u.uOff, this.imgOff[0], this.imgOff[1]);
    gl.uniform2f(pr.u.uSz, this.imgSz[0], this.imgSz[1]);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.bgTex);
    gl.uniform1i(pr.u.uTex, 0);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    if (this.vis <= 0.002) return;
    const sc = 1 + 0.08 * (1 - this.vis);
    const pw = PW * DPR * sc, ph = PH * DPR * sc;
    const px = CX * DPR - pw / 2, py = CY * DPR - ph / 2;
    const rx = CX * DPR - REGION_W / 2, ry = CY * DPR - REGION_H / 2;
    // проход 1: bg → FBO A (горизонталь, в uv-пространстве обоев)
    const step1x = (18 * DPR) / this.imgSz[0];
    gl.viewport(0, 0, HALF_W, HALF_H);
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.fboA.f);
    pr = this.progBlur;
    gl.useProgram(pr.p);
    gl.enableVertexAttribArray(pr.aPos);
    gl.vertexAttribPointer(pr.aPos, 2, gl.FLOAT, false, 0, 0);
    const o1x = (rx - this.imgOff[0]) / this.imgSz[0];
    const o1y = (ry - this.imgOff[1]) / this.imgSz[1];
    gl.uniform4f(pr.u.uOriginSize, o1x, o1y, REGION_W / this.imgSz[0],
      REGION_H / this.imgSz[1]);
    gl.uniform2f(pr.u.uStep, step1x, 0);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.bgTex);
    gl.uniform1i(pr.u.uTex, 0);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    // проход 2: FBO A → FBO B (вертикаль, uv региона)
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.fboB.f);
    gl.uniform4f(pr.u.uOriginSize, 0, 0, 1, 1);
    gl.uniform2f(pr.u.uStep, 0, (9 * DPR) / HALF_H);
    gl.bindTexture(gl.TEXTURE_2D, this.fboA.t);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    // composite: панель поверх фона
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, W, H);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    pr = this.progPanel;
    gl.useProgram(pr.p);
    gl.enableVertexAttribArray(pr.aPos);
    gl.vertexAttribPointer(pr.aPos, 2, gl.FLOAT, false, 0, 0);
    gl.uniform2f(pr.u.uRes, W, H);
    gl.uniform4f(pr.u.uRect, px, py, pw, ph);
    gl.uniform4f(pr.u.uRegion, rx, ry, REGION_W, REGION_H);
    gl.uniform1f(pr.u.uRadius, 14 * DPR * sc);
    gl.uniform1f(pr.u.uOpacity, this.vis);
    gl.uniform4f(pr.u.uTint, TINT[0], TINT[1], TINT[2], TINT[3]);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.fboB.t);
    gl.uniform1i(pr.u.uBlur, 0);
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this.contentTex);
    gl.uniform1i(pr.u.uContent, 1);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  }
}
//── heavy-сцена: DOM со сложностью лаунчера ──
// keyframes settingsRise/Sink один в один из src/index.html: scale 1.08 +
// opacity + element-filter blur(14px), 0.4s cubic-bezier(0.4,0,0.2,1).
// Element-blur остаётся во всех режимах (кроме контрольного hv-noblur) —
// как в лаунчере («глубина остаётся во всех режимах»).
const hvScene = document.getElementById('hvScene');
const hvPanel = document.getElementById('hvPanel');
const hvRight = document.getElementById('hvRight');
const hvBaked = document.getElementById('hvBaked');

const KF_RISE = [
  { opacity: 0, transform: 'translate(-50%,-50%) scale(1.08)', filter: 'blur(14px)' },
  { opacity: 1, transform: 'translate(-50%,-50%) scale(1)',    filter: 'blur(0)' },
];
const KF_SINK = [
  { opacity: 1, transform: 'translate(-50%,-50%) scale(1)',    filter: 'blur(0)' },
  { opacity: 0, transform: 'translate(-50%,-50%) scale(1.08)', filter: 'blur(14px)' },
];
// hv-noblur (пол): только вылет/затухание, без element-filter
const stripF = kf => kf.map(k => ({ opacity: k.opacity, transform: k.transform }));
const KF_RISE_NB = stripF(KF_RISE), KF_SINK_NB = stripF(KF_SINK);

let hvRiseT = 0, hvSinkT = 0, hvV22T = 0;

// Идея №1: запечённый фон-блюр — размытая копия обоев под панелью,
// рисуется один раз в setup(). Обои cover-ятся по вьюпорту и тайлятся
// 3×3, чтобы у границ холста не осталось прозрачности; холст больше
// панели на 3σ (PADF) — фейд гаусса у его кромки остаётся за кромкой
// панели (панель обрезает холст через overflow:hidden).
function bakeHvBg() {
  const pw = Math.min(940, VW * 0.95), ph = Math.min(VH * 0.86, 680);
  const CW = Math.round(pw + 2 * PADF), CH = Math.round(ph + 2 * PADF);
  const W = Math.round(CW * DPR), H = Math.round(CH * DPR);
  const tmp = document.createElement('canvas');
  tmp.width = W; tmp.height = H;
  const tx = tmp.getContext('2d');
  const s = Math.max(VW / IMG.width, VH / IMG.height);
  const iw = IMG.width * s * DPR, ih = IMG.height * s * DPR;
  const ox = (W - iw) / 2, oy = (H - ih) / 2;
  for (let ky = -1; ky <= 1; ky++)
    for (let kx = -1; kx <= 1; kx++)
      tx.drawImage(IMG, ox + kx * iw, oy + ky * ih, iw, ih);
  hvBaked.width = W; hvBaked.height = H;
  hvBaked.style.width = CW + 'px';
  hvBaked.style.height = CH + 'px';
  const bx = hvBaked.getContext('2d');
  bx.filter = `blur(${BLUR * DPR}px) saturate(1.2)`;
  bx.drawImage(tmp, 0, 0);
  bx.filter = 'none';
  bx.fillStyle = 'rgba(11,13,20,0.7)';        // тинт панели поверх блюра
  bx.fillRect(0, 0, W, H);
}

// Открытие/закрытие — тайминги из openSettings/closeSettings/_closeWithAnim:
// display+news-away+anim-rise на старте (anim-rise снимают через 500мс),
// V22 (settings-anim) держится 550мс, при закрытии язычок возвращается
// сразу (показ интерфейса в _closeWithAnim), closing+display:none через 440мс.
function hvAnimate(dir, o) {
  hvPanel.getAnimations().forEach(a => a.cancel());
  clearTimeout(hvRiseT); clearTimeout(hvSinkT); clearTimeout(hvV22T);
  if (dir) {
    hvPanel.style.display = 'flex';
    hvRight.classList.add('news-away');
    hvPanel.classList.remove('closing');
    hvPanel.classList.add('anim-rise');
    hvRiseT = setTimeout(() => hvPanel.classList.remove('anim-rise'), 500);
    hvPanel.animate(o.noblur ? KF_RISE_NB : KF_RISE,
      { duration: DUR, easing: 'cubic-bezier(0.4,0,0.2,1)', fill: 'forwards' });
  } else {
    hvRight.classList.remove('news-away');
    hvPanel.classList.remove('anim-rise');
    hvPanel.classList.add('closing');
    hvPanel.animate(o.noblur ? KF_SINK_NB : KF_SINK,
      { duration: DUR, easing: 'cubic-bezier(0.4,0,0.2,1)', fill: 'forwards' });
    hvSinkT = setTimeout(() => {
      hvPanel.classList.remove('closing');
      hvPanel.style.display = 'none';
    }, 440);
  }
  if (o.v22) {
    hvScene.classList.add('settings-anim');
    hvV22T = setTimeout(() => hvScene.classList.remove('settings-anim'), 550);
  }
}

function hvVariant(name, o) {
  const renderer = o.noblur ? 'DOM без блюров (пол)'
    : o.baked ? 'DOM + запечённый фон-блюр (идея №1)'
    : o.smooth ? 'DOM, backdrop выкл на время анимации (smooth)'
    : o.v22 === false ? 'DOM full без V22 (внешние блюры живут)'
    : 'DOM + backdrop live + element-blur (full)';
  return {
    name, renderer,
    async setup() { if (o.baked) bakeHvBg(); },
    show() {
      hvScene.classList.add('on');
      hvScene.classList.toggle('m-baked', !!o.baked);
      hvScene.classList.toggle('m-smooth', !!o.smooth);
      hvScene.classList.toggle('m-noblur', !!o.noblur);
      hvScene.classList.remove('settings-anim');
      hvRight.classList.remove('news-away');
      hvPanel.getAnimations().forEach(a => a.cancel());
      hvPanel.classList.remove('anim-rise', 'closing');
      hvPanel.style.display = 'none';         // закрытое состояние до циклов
    },
    hide() {
      hvPanel.getAnimations().forEach(a => a.cancel());
      clearTimeout(hvRiseT); clearTimeout(hvSinkT); clearTimeout(hvV22T);
      hvScene.classList.remove('on', 'settings-anim', 'm-baked', 'm-smooth', 'm-noblur');
      hvPanel.classList.remove('anim-rise', 'closing');
      hvPanel.style.display = 'none';
      hvRight.classList.remove('news-away');
    },
    open() { hvAnimate(1, o); },
    close() { hvAnimate(0, o); },
  };
}

//── замер: сбор кадров rAF + longtask, фазы как в perf-anim-ab ──
function collectStart() {
  const frames = [];
  const longTasks = [];
  let last = 0, raf = 0, on = true;
  let po = null;
  try {
    po = new PerformanceObserver(l => {
      for (const e of l.getEntries()) longTasks.push(Math.round(e.duration));
    });
    po.observe({ entryTypes: ['longtask'] });
  } catch (e) { /* нет longtask — не беда */ }
  const tick = () => {
    if (!on) return;
    const n = performance.now();
    if (last) frames.push({ t: n, dt: n - last });
    last = n;
    raf = requestAnimationFrame(tick);
  };
  raf = requestAnimationFrame(tick);
  return {
    stop() {
      on = false;
      cancelAnimationFrame(raf);
      try { po && po.disconnect(); } catch (e) {}
      return { frames, longTasks };
    },
  };
}

function stat(a) {
  if (!a || !a.length) return null;
  const s = a.slice().sort((x, y) => x - y);
  const pct = p => +(s[Math.min(s.length - 1, Math.floor(s.length * p))] || 0).toFixed(1);
  return {
    n: a.length,
    avgMs: +(a.reduce((x, y) => x + y, 0) / a.length).toFixed(1),
    p95Ms: pct(0.95), maxMs: pct(1),
    janky33: a.filter(f => f > 33).length,
    over50: a.filter(f => f > 50).length,
  };
}

async function cycleOnce(v) {
  v.open();
  await sleep(700);
  v.close();
  await sleep(700);
}

async function measure(v, cycles) {
  const col = collectStart();
  const marks = [];
  for (let i = 0; i < cycles; i++) {
    marks.push({ kind: 'open', t: performance.now() });
    v.open();
    await sleep(700);
    marks.push({ kind: 'close', t: performance.now() });
    v.close();
    await sleep(700);
  }
  const { frames, longTasks } = col.stop();
  const phase = t => {
    let last = null;
    for (const m of marks) { if (m.t <= t) last = m; }
    if (!last) return 'before';
    const d = t - last.t;
    if (last.kind === 'open') return d < 400 ? 'open-anim' : 'open-static';
    return d < 400 ? 'close-anim' : 'after';
  };
  const phases = {};
  for (const f of frames) {
    if (f.dt > 500) continue;
    const p = phase(f.t);
    (phases[p] = phases[p] || []).push(f.dt);
  }
  const all = Object.values(phases).flat();
  const s = all.slice().sort((a, b) => a - b);
  const pct = p => +(s[Math.min(s.length - 1, Math.floor(s.length * p))] || 0).toFixed(1);
  return {
    frames: all.length,
    avgMs: +(all.length ? all.reduce((a, b) => a + b, 0) / all.length : 0).toFixed(1),
    p50Ms: pct(0.5), p95Ms: pct(0.95), maxMs: pct(1),
    janky33: all.filter(f => f > 33).length,
    over50: all.filter(f => f > 50).length,
    over100: all.filter(f => f > 100).length,
    openAnim: stat(phases['open-anim']),
    closeAnim: stat(phases['close-anim']),
    longTasks: longTasks.length,
    longestMs: longTasks.reduce((a, b) => Math.max(a, b), 0),
  };
}

//── наборы вариантов ──
const SETS = {
  // DOM-сцена со сложностью лаунчера (см. заголовок)
  heavy: [
    hvVariant('hv-full', { v22: true }),
    hvVariant('hv-baked', { baked: true, v22: true }),
    hvVariant('hv-smooth', { smooth: true, v22: true }),
    hvVariant('hv-noblur', { noblur: true, v22: true }),
    hvVariant('hv-full-nov22', { v22: false }),
  ],
  // простая сцена, рендер-пути (исходный набор стенда)
  light: [
    domVariant('dom-blur', false),
    domVariant('dom-noblur', true),
    new C2D('cv2d', false),
    new C2D('cvbaked', true),
    new WGL(),
  ],
};

window.runBench = async function (cycles, set) {
  set = SETS[set] ? set : 'heavy';
  const list = SETS[set];
  await IMGREADY;
  const res = {
    meta: {
      vw: VW, vh: VH, dpr: DPR, pw: PW, ph: PH,
      hpw: Math.round(Math.min(940, VW * 0.95)),
      hph: Math.round(Math.min(VH * 0.86, 680)),
      blur: BLUR, cycles, set,
    },
    variants: {},
  };
  for (const v of list) {
    hud.textContent = 'setup: ' + v.name;
    try {
      await v.setup();
      v.show();
      hud.textContent = 'warmup: ' + v.name;
      await cycleOnce(v);
      hud.textContent = 'measure: ' + v.name;
      const r = await measure(v, cycles);
      r.renderer = v.renderer;
      res.variants[v.name] = r;
      console.log('[bench] ' + v.name + ' avg=' + r.avgMs + 'ms >33=' + r.janky33);
    } catch (e) {
      res.variants[v.name] = { error: String((e && e.message) || e) };
      console.log('[bench] ERROR ' + v.name + ': ' + ((e && e.message) || e));
    }
    try { v.hide(); } catch (e) {}
    await sleep(500);
  }
  hud.textContent = 'готово';
  return res;
};
