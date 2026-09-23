// @ts-check
/*
 * Impulcifer interactive report renderer: offline, no dependencies.
 *
 * Data: every tab owns one base64 little-endian Float32 blob, decoded the first
 * time the tab opens (then the source text is dropped). Series address it by
 * [offset, length]; evenly sampled series carry their x as (i + start) * step.
 * Drawing: 2D canvas at devicePixelRatio. All dirty charts redraw in one
 * requestAnimationFrame; each line keeps at most four vertices per device-pixel
 * column (first, min, max, last: M4), so a redraw costs one pass over the
 * visible samples and a path proportional to the plot width. Charts outside the
 * viewport are skipped (IntersectionObserver) and sizes follow ResizeObserver.
 * Hover draws on a second canvas so the data layer is not redrawn per move.
 */
(function () {
  "use strict";

  /**
   * @typedef {{label: string, unit?: string, log?: boolean, view?: number[] | null, ticks?: number[] | null, digits?: number, sig?: number}} AxisSpec
   * @typedef {{name: string, color: string, alpha?: number, width?: number, dash?: boolean, y: number[], x?: number[] | null, start?: number, step?: number, fill?: boolean, hover?: boolean}} SeriesSpec
   * @typedef {{label: string, detail: string, value: number}} BarSpec
   * @typedef {{x: number, y: number, color: string}} PointSpec
   * @typedef {{kind: string, title: string, subtitle?: string, wide?: boolean, size?: string, link?: string, x: AxisSpec, y: AxisSpec, series?: SeriesSpec[], bars?: BarSpec[], guides?: number[], zero?: boolean, points?: PointSpec[]}} ChartSpec
   * @typedef {{id: string, title: string, blob?: string | null, floats?: number, note?: string, charts: ChartSpec[]}} TabSpec
   * @typedef {{title: string, subtitle: string, generator: string, tabs: TabSpec[]}} Manifest
   * @typedef {{spec: SeriesSpec, ys: Float32Array, xs: Float32Array | null, start: number, step: number, n: number, visible: boolean}} Series
   * @typedef {{x0: number, x1: number, y0: number, y1: number}} View
   * @typedef {{l: number, t: number, r: number, b: number}} Rect
   */

  const manifestNode = document.getElementById("report-manifest");
  /** @type {Manifest} */
  const manifest = JSON.parse((manifestNode && manifestNode.textContent) || "{}");

  const stats = { frames: 0, draws: 0, lastFrameMs: 0, totalDrawMs: 0, decodeMs: 0 };
  /** @type {any} */ (window).impulciferReport = { stats: stats };

  // ---------------------------------------------------------------- data

  const littleEndian = new Uint8Array(new Uint16Array([1]).buffer)[0] === 1;

  /** @param {string} text */
  function decodeBase64(text) {
    const clean = text.trim();
    const native = /** @type {any} */ (Uint8Array).fromBase64;
    if (typeof native === "function") return /** @type {Uint8Array} */ (native(clean));
    const binary = atob(clean);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
    return bytes;
  }

  /** @param {string | null | undefined} id */
  function loadBlob(id) {
    if (!id) return new Float32Array(0);
    const node = document.getElementById(id);
    if (!node) return new Float32Array(0);
    const started = performance.now();
    let bytes = decodeBase64(node.textContent || "");
    node.remove();
    if (bytes.byteOffset % 4 !== 0) bytes = bytes.slice();
    let floats;
    if (littleEndian) {
      floats = new Float32Array(bytes.buffer, bytes.byteOffset, bytes.byteLength >> 2);
    } else {
      const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      floats = new Float32Array(bytes.byteLength >> 2);
      for (let i = 0; i < floats.length; i++) floats[i] = view.getFloat32(4 * i, true);
    }
    stats.decodeMs += performance.now() - started;
    return floats;
  }

  // ---------------------------------------------------------------- theme

  /** @type {Record<string, string>} */
  let theme = {};
  function readTheme() {
    const style = getComputedStyle(document.documentElement);
    const names = ["ink", "muted", "grid", "grid-minor", "zero", "left", "right", "diff", "pos-fill", "neg-fill", "panel", "font"];
    /** @type {Record<string, string>} */
    const next = {};
    for (const name of names) next[name] = style.getPropertyValue("--" + name).trim();
    theme = next;
  }
  readTheme();
  /** @param {string} role */
  function color(role) {
    return theme[role] || role;
  }

  // ---------------------------------------------------------------- scheduling

  /** @type {Set<Chart>} */
  const dirty = new Set();
  /** @type {Set<Chart>} */
  const hoverDirty = new Set();
  let raf = 0;
  function request() {
    if (!raf) raf = requestAnimationFrame(frame);
  }
  function frame() {
    raf = 0;
    const started = performance.now();
    for (const chart of dirty) chart.draw();
    dirty.clear();
    for (const chart of hoverDirty) chart.drawHover();
    hoverDirty.clear();
    stats.frames++;
    stats.lastFrameMs = performance.now() - started;
  }
  /** @param {Chart} chart */
  function schedule(chart) {
    chart.needsDraw = true;
    if (chart.visible && chart.width > 0) {
      dirty.add(chart);
      request();
    }
  }

  const resizer = new ResizeObserver((entries) => {
    for (const entry of entries) {
      const chart = charts.get(entry.target);
      if (chart) chart.resize(entry.contentRect.width, entry.contentRect.height);
    }
  });
  const watcher = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        const chart = charts.get(entry.target);
        if (!chart) continue;
        chart.visible = entry.isIntersecting;
        if (chart.visible && chart.needsDraw) schedule(chart);
      }
    },
    { rootMargin: "200px 0px" },
  );
  /** @type {Map<Element, Chart>} */
  const charts = new Map();
  /** @type {Map<string, Chart[]>} */
  const links = new Map();

  // ---------------------------------------------------------------- numbers

  const MINUS = "−";
  /** @param {number} value @param {number} digits */
  function fixed(value, digits) {
    if (!isFinite(value)) return "–";
    const abs = Math.abs(value);
    let text;
    if (abs !== 0 && (abs >= 1e6 || abs < Math.pow(10, -Math.max(digits, 4)))) text = value.toExponential(2);
    else text = value.toFixed(digits);
    if (/^-0(\.0*)?$/.test(text)) text = text.slice(1);
    return text.replace("-", MINUS);
  }
  /** @param {number} value @param {AxisSpec} axis @param {number} fallback */
  function readout(value, axis, fallback) {
    if (axis.sig && isFinite(value) && value !== 0) {
      const text = Math.abs(value) < 1e-4 ? value.toExponential(axis.sig - 1) : value.toPrecision(axis.sig);
      return text.replace("-", MINUS);
    }
    return fixed(value, axis.digits === undefined ? fallback : axis.digits);
  }
  /** @param {number} step */
  function decimals(step) {
    return Math.max(0, Math.min(10, -Math.floor(Math.log10(step) + 1e-9)));
  }
  /** @param {number} value @param {number} step */
  function tickText(value, step) {
    if (Math.abs(value) < step * 1e-6) value = 0;
    return fixed(value, decimals(step));
  }
  /** @param {number} value */
  function hzText(value) {
    if (value >= 1000) {
      const k = value / 1000;
      return +k.toFixed(k < 10 ? 2 : 1) + "k";
    }
    return String(+value.toFixed(value < 10 ? 2 : value < 100 ? 1 : 0));
  }
  /** @param {number} span @param {number} count */
  function niceStep(span, count) {
    const raw = span / Math.max(1, count);
    const power = Math.pow(10, Math.floor(Math.log10(raw)));
    const f = raw / power;
    return (f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10) * power;
  }
  /** @param {number} lo @param {number} hi @param {number} count @returns {{ticks: number[], step: number}} */
  function linearTicks(lo, hi, count) {
    const step = niceStep(hi - lo, count);
    const ticks = [];
    const first = Math.ceil(lo / step - 1e-9);
    for (let k = first; k * step <= hi + step * 1e-9 && ticks.length < 200; k++) ticks.push(k * step);
    return { ticks: ticks, step: step };
  }
  /** @param {number} lo @param {number} hi @param {number} pixels */
  function logTicks(lo, hi, pixels) {
    const decades = Math.log10(hi) - Math.log10(lo);
    const perDecade = pixels / Math.max(decades, 1e-9);
    const mantissas = perDecade > 110 ? [1, 2, 5] : perDecade > 55 ? [1, 3] : [1];
    const major = [];
    const minor = [];
    for (let e = Math.floor(Math.log10(lo)); e <= Math.ceil(Math.log10(hi)); e++) {
      const base = Math.pow(10, e);
      for (let m = 1; m < 10; m++) {
        const v = m * base;
        if (v < lo * (1 - 1e-9) || v > hi * (1 + 1e-9)) continue;
        if (mantissas.indexOf(m) >= 0) major.push(v);
        else minor.push(v);
      }
    }
    if (major.length < 2) return { major: linearTicks(lo, hi, Math.max(2, pixels / 90)).ticks, minor: [] };
    return { major: major, minor: perDecade > 40 ? minor : [] };
  }

  // ---------------------------------------------------------------- dom

  /**
   * @template {keyof HTMLElementTagNameMap} K
   * @param {K} tag @param {string} [className] @param {string} [text]
   * @returns {HTMLElementTagNameMap[K]}
   */
  function el(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  }
  /** @param {string} text */
  function escapeHtml(text) {
    return text.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] || c);
  }

  // ---------------------------------------------------------------- chart

  const DPR_LIMIT = 3;

  class Chart {
    /** @param {ChartSpec} spec @param {Float32Array} data @param {HTMLElement} host */
    constructor(spec, data, host) {
      this.spec = spec;
      this.bars = spec.bars || null;
      this.logX = !!spec.x.log;
      /** @type {Series[]} */
      this.series = (spec.series || []).map((s) => {
        const ys = data.subarray(s.y[0], s.y[0] + s.y[1]);
        const xs = s.x ? data.subarray(s.x[0], s.x[0] + s.x[1]) : null;
        return { spec: s, ys: ys, xs: xs, start: s.start || 0, step: s.step || 1, n: ys.length, visible: true };
      });
      this.visible = false;
      this.needsDraw = true;
      this.width = 0;
      this.height = 0;
      this.dpr = 1;
      /** @type {Rect | null} */
      this.rect = null;
      /** @type {{x: number, y: number} | null} */
      this.pointer = null;
      /** @type {{x: number, y: number, view: View} | null} */
      this.drag = null;

      const card = el("section", "card" + (spec.wide ? " wide" : ""));
      const head = el("div", "card-head");
      const titles = el("div");
      titles.appendChild(el("h2", "", spec.title));
      if (spec.subtitle) titles.appendChild(el("p", "sub", spec.subtitle));
      head.appendChild(titles);
      if (!this.bars && this.series.length) head.appendChild(this.legend());
      card.appendChild(head);
      const plot = el("div", "plot" + (spec.size ? " " + spec.size : "") + (this.bars ? "" : " pannable"));
      plot.tabIndex = 0;
      plot.setAttribute("role", "img");
      plot.setAttribute("aria-label", spec.title + (this.bars ? "" : ". Drag to pan, wheel to zoom, double-click to reset."));
      this.base = el("canvas");
      this.over = el("canvas");
      this.tip = el("div", "tip");
      this.tip.hidden = true;
      plot.append(this.base, this.over, this.tip);
      card.appendChild(plot);
      host.appendChild(card);
      this.plot = plot;
      const base = this.base.getContext("2d");
      const over = this.over.getContext("2d");
      if (!base || !over) throw new Error("canvas 2d unavailable");
      this.ctx = base;
      this.octx = over;

      this.home = this.homeView();
      /** @type {View} */
      this.view = Object.assign({}, this.home);
      /** x extent that zooming and panning keep in reach (data plus home view), in axis units */
      this.extent = this.xExtent();
      if (spec.link) {
        const group = links.get(spec.link) || [];
        group.push(this);
        links.set(spec.link, group);
      }
      this.listen();
      charts.set(plot, this);
      resizer.observe(plot);
      watcher.observe(plot);
    }

    legend() {
      const legend = el("div", "legend");
      for (const s of this.series) {
        const button = el("button");
        button.type = "button";
        button.setAttribute("aria-pressed", "true");
        const swatch = el("span", "swatch" + (s.spec.dash ? " dashed" : "") + ((s.spec.alpha || 1) < 0.6 ? " faint" : ""));
        swatch.dataset.role = s.spec.color;
        button.append(swatch, document.createTextNode(s.spec.name));
        button.addEventListener("click", () => {
          s.visible = !s.visible;
          button.setAttribute("aria-pressed", String(s.visible));
          schedule(this);
        });
        legend.appendChild(button);
      }
      this.swatches = legend;
      this.paintSwatches();
      return legend;
    }

    paintSwatches() {
      if (!this.swatches) return;
      for (const node of this.swatches.querySelectorAll(".swatch")) {
        /** @type {HTMLElement} */ (node).style.color = color(/** @type {HTMLElement} */ (node).dataset.role || "ink");
      }
    }

    // ------------------------------------------------------------ geometry

    /** @param {Series} s @param {number} i */
    xAt(s, i) {
      return s.xs ? s.xs[i] : (i + s.start) * s.step;
    }

    /** @param {number} v */
    tx(v) {
      return this.logX ? Math.log10(v) : v;
    }
    /** @param {number} t */
    fromT(t) {
      return this.logX ? Math.pow(10, t) : t;
    }

    /** @returns {View} */
    homeView() {
      const spec = this.spec;
      if (this.bars) {
        let lo = 0;
        let hi = 0;
        for (const bar of this.bars) {
          lo = Math.min(lo, bar.value);
          hi = Math.max(hi, bar.value);
        }
        const y = spec.y.view || (hi - lo < 1e-9 ? [-1, 1] : padded(lo, hi, 0.12));
        return { x0: 0, x1: this.bars.length, y0: y[0], y1: y[1] };
      }
      let x0 = Infinity;
      let x1 = -Infinity;
      for (const s of this.series) {
        if (!s.n) continue;
        x0 = Math.min(x0, this.xAt(s, 0));
        x1 = Math.max(x1, this.xAt(s, s.n - 1));
      }
      if (spec.x.view) {
        x0 = spec.x.view[0];
        x1 = spec.x.view[1];
      }
      if (!(x1 > x0)) {
        x0 = isFinite(x0) ? x0 - 1 : 0;
        x1 = x0 + 2;
      }
      if (spec.y.view) return { x0: x0, x1: x1, y0: spec.y.view[0], y1: spec.y.view[1] };
      let lo = Infinity;
      let hi = -Infinity;
      for (const s of this.series) {
        const [i0, i1] = this.range(s, x0, x1);
        for (let i = i0; i <= i1; i++) {
          const x = this.xAt(s, i);
          if (x < x0 || x > x1) continue;
          const v = s.ys[i];
          if (v < lo) lo = v;
          if (v > hi) hi = v;
        }
      }
      for (const g of spec.guides || []) {
        lo = Math.min(lo, g);
        hi = Math.max(hi, g);
      }
      const y = padded(lo, hi, 0.08);
      return { x0: x0, x1: x1, y0: y[0], y1: y[1] };
    }

    xExtent() {
      let t0 = this.tx(this.home.x0);
      let t1 = this.tx(this.home.x1);
      for (const s of this.series) {
        if (!s.n) continue;
        const a = this.xAt(s, 0);
        const b = this.xAt(s, s.n - 1);
        if (!this.logX || a > 0) t0 = Math.min(t0, this.tx(a));
        if (!this.logX || b > 0) t1 = Math.max(t1, this.tx(b));
      }
      return { t0: t0, t1: t1 };
    }

    /** Keep an x view (axis units) within 5% beyond the extent and no wider than it. @param {number} n0 @param {number} n1 */
    clampX(n0, n1) {
      const e = this.extent;
      const margin = (e.t1 - e.t0) * 0.05;
      const lo = e.t0 - margin;
      const hi = e.t1 + margin;
      let span = n1 - n0;
      if (span > hi - lo) {
        span = hi - lo;
        const c = (n0 + n1) / 2;
        n0 = c - span / 2;
        n1 = c + span / 2;
      }
      if (n0 < lo) {
        n1 += lo - n0;
        n0 = lo;
      }
      if (n1 > hi) {
        n0 -= n1 - hi;
        n1 = hi;
      }
      return [n0, n1];
    }

    /** Index range covering [x0, x1] plus one sample on each side. @param {Series} s @param {number} x0 @param {number} x1 */
    range(s, x0, x1) {
      if (!s.n) return [0, -1];
      let i0;
      let i1;
      if (s.xs) {
        i0 = lowerBound(s.xs, x0) - 1;
        i1 = lowerBound(s.xs, x1);
      } else {
        i0 = Math.floor(x0 / s.step - s.start) - 1;
        i1 = Math.ceil(x1 / s.step - s.start) + 1;
      }
      return [Math.max(0, i0), Math.min(s.n - 1, i1)];
    }

    /** @param {Series} s @param {number} x */
    nearest(s, x) {
      if (!s.n) return -1;
      if (!s.xs) return Math.max(0, Math.min(s.n - 1, Math.round(x / s.step - s.start)));
      const i = lowerBound(s.xs, x);
      if (i <= 0) return 0;
      if (i >= s.n) return s.n - 1;
      const a = this.tx(s.xs[i - 1]);
      const b = this.tx(s.xs[i]);
      const t = this.tx(x);
      return t - a <= b - t ? i - 1 : i;
    }

    /** @param {number} width @param {number} height */
    resize(width, height) {
      const dpr = Math.min(window.devicePixelRatio || 1, DPR_LIMIT);
      if (width === this.width && height === this.height && dpr === this.dpr) return;
      this.width = width;
      this.height = height;
      this.dpr = dpr;
      for (const canvas of [this.base, this.over]) {
        canvas.width = Math.max(1, Math.round(width * dpr));
        canvas.height = Math.max(1, Math.round(height * dpr));
      }
      schedule(this);
    }

    // ------------------------------------------------------------ drawing

    draw() {
      this.needsDraw = false;
      const dpr = Math.min(window.devicePixelRatio || 1, DPR_LIMIT);
      if (dpr !== this.dpr) {
        this.resize(this.width, this.height);
        this.needsDraw = false;
      }
      if (this.width <= 0 || this.height <= 0) return;
      const started = performance.now();
      const ctx = this.ctx;
      ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
      ctx.clearRect(0, 0, this.width, this.height);
      ctx.font = "11px " + (theme.font || "sans-serif");
      ctx.textBaseline = "middle";
      const v = this.view;
      const yt = this.spec.y.ticks
        ? { ticks: this.spec.y.ticks.filter((t) => t >= v.y0 - 1e-9 && t <= v.y1 + 1e-9), step: 45 }
        : linearTicks(v.y0, v.y1, Math.max(2, this.height / 48));
      const ylabels = yt.ticks.map((t) => tickText(t, yt.step));
      let labelWidth = 0;
      for (const label of ylabels) labelWidth = Math.max(labelWidth, ctx.measureText(label).width);
      // A common minimum gutter keeps stacked and linked charts aligned.
      const rect = { l: Math.max(Math.ceil(labelWidth) + 30, 58), t: 8, r: this.width - 12, b: this.height - 40 };
      if (rect.r - rect.l < 20 || rect.b - rect.t < 20) return;
      this.rect = rect;
      const sx = this.xMapper(rect);
      const sy = this.yMapper(rect);

      // Grid and x ticks.
      ctx.lineWidth = 1;
      /** @type {{v: number, label: string}[]} */
      let xticks = [];
      /** @type {number[]} */
      let xminor = [];
      if (this.bars) {
        xticks = [];
      } else if (this.logX) {
        const ticks = logTicks(v.x0, v.x1, rect.r - rect.l);
        xticks = ticks.major.map((t) => ({ v: t, label: hzText(t) }));
        xminor = ticks.minor;
      } else {
        const ticks = linearTicks(v.x0, v.x1, Math.max(2, (rect.r - rect.l) / 80));
        xticks = ticks.ticks.map((t) => ({ v: t, label: tickText(t, ticks.step) }));
      }
      ctx.strokeStyle = color("grid-minor");
      ctx.beginPath();
      for (const t of xminor) {
        const x = Math.round(sx(t)) + 0.5;
        ctx.moveTo(x, rect.t);
        ctx.lineTo(x, rect.b);
      }
      ctx.stroke();
      ctx.strokeStyle = color("grid");
      ctx.beginPath();
      for (const t of xticks) {
        const x = Math.round(sx(t.v)) + 0.5;
        ctx.moveTo(x, rect.t);
        ctx.lineTo(x, rect.b);
      }
      for (const t of yt.ticks) {
        const y = Math.round(sy(t)) + 0.5;
        ctx.moveTo(rect.l, y);
        ctx.lineTo(rect.r, y);
      }
      ctx.stroke();

      ctx.save();
      ctx.beginPath();
      ctx.rect(rect.l, rect.t, rect.r - rect.l, rect.b - rect.t);
      ctx.clip();
      if (this.spec.zero && v.y0 < 0 && v.y1 > 0) {
        ctx.strokeStyle = color("zero");
        ctx.beginPath();
        const y = Math.round(sy(0)) + 0.5;
        ctx.moveTo(rect.l, y);
        ctx.lineTo(rect.r, y);
        ctx.stroke();
      }
      if (this.spec.guides) {
        ctx.strokeStyle = color("zero");
        ctx.setLineDash([4, 4]);
        ctx.beginPath();
        for (const g of this.spec.guides) {
          const y = Math.round(sy(g)) + 0.5;
          ctx.moveTo(rect.l, y);
          ctx.lineTo(rect.r, y);
        }
        ctx.stroke();
        ctx.setLineDash([]);
      }
      if (this.bars) this.drawBars(sy, rect);
      else this.drawLines(sx, sy, rect);
      ctx.restore();

      // Axes, labels, titles.
      ctx.strokeStyle = color("zero");
      ctx.beginPath();
      ctx.moveTo(rect.l + 0.5, rect.t);
      ctx.lineTo(rect.l + 0.5, rect.b + 0.5);
      ctx.lineTo(rect.r, rect.b + 0.5);
      ctx.stroke();
      ctx.fillStyle = color("muted");
      ctx.textAlign = "right";
      yt.ticks.forEach((t, i) => ctx.fillText(ylabels[i], rect.l - 6, sy(t)));
      ctx.textAlign = "center";
      ctx.textBaseline = "top";
      if (this.bars) {
        const band = (rect.r - rect.l) / this.bars.length;
        const step = Math.ceil(44 / band);
        this.bars.forEach((bar, i) => {
          if (i % step === 0) ctx.fillText(bar.label, rect.l + band * (i + 0.5), rect.b + 6);
        });
      } else {
        let last = -Infinity;
        for (const t of xticks) {
          const x = sx(t.v);
          const w = ctx.measureText(t.label).width;
          if (x - w / 2 < last + 6) continue;
          ctx.fillText(t.label, x, rect.b + 6);
          last = x + w / 2;
        }
      }
      ctx.fillStyle = color("ink");
      ctx.font = "12px " + (theme.font || "sans-serif");
      ctx.fillText(this.spec.x.label, (rect.l + rect.r) / 2, rect.b + 22);
      ctx.save();
      ctx.translate(12, (rect.t + rect.b) / 2);
      ctx.rotate(-Math.PI / 2);
      ctx.textBaseline = "middle";
      ctx.fillText(this.spec.y.label, 0, 0);
      ctx.restore();

      stats.draws++;
      stats.totalDrawMs += performance.now() - started;
      this.drawHover();
    }

    /** @param {Rect} rect */
    xMapper(rect) {
      const v = this.view;
      if (this.bars) {
        const k = (rect.r - rect.l) / (v.x1 - v.x0);
        return (/** @type {number} */ x) => rect.l + (x - v.x0) * k;
      }
      const t0 = this.tx(v.x0);
      const k = (rect.r - rect.l) / (this.tx(v.x1) - t0);
      if (this.logX) return (/** @type {number} */ x) => rect.l + (Math.log10(x) - t0) * k;
      return (/** @type {number} */ x) => rect.l + (x - t0) * k;
    }
    /** @param {Rect} rect */
    yMapper(rect) {
      const v = this.view;
      const k = (rect.b - rect.t) / (v.y1 - v.y0);
      return (/** @type {number} */ y) => rect.b - (y - v.y0) * k;
    }

    /**
     * M4 decimation of one series into device-column vertices.
     * @param {Series} s @param {(x: number) => number} sx @param {(y: number) => number} sy
     * @returns {number[][]} polylines as flat [x0, y0, x1, y1, ...] arrays
     */
    decimate(s, sx, sy) {
      const v = this.view;
      const [i0, i1] = this.range(s, v.x0, v.x1);
      const dpr = this.dpr;
      const ys = s.ys;
      const lines = [];
      /** @type {number[]} */
      let line = [];
      let column = NaN;
      let count = 0;
      let fx = 0, fy = 0, fi = 0, lx = 0, ly = 0, li = 0;
      let nx = 0, ny = 0, ni = 0, mx = 0, my = 0, mi = 0;
      const flush = () => {
        if (!count) return;
        line.push(fx, fy);
        if (count > 1) {
          if (ni < mi) {
            if (ni !== fi) line.push(nx, ny);
            if (mi !== li) line.push(mx, my);
          } else {
            if (mi !== fi) line.push(mx, my);
            if (ni !== li) line.push(nx, ny);
          }
          line.push(lx, ly);
        }
        count = 0;
      };
      // Evenly sampled series on a linear axis: x is affine in i, no per-sample call.
      const affine = !s.xs && !this.logX;
      const a = affine ? sx(s.start * s.step) : 0;
      const b = affine ? sx((1 + s.start) * s.step) - a : 0;
      for (let i = i0; i <= i1; i++) {
        const value = ys[i];
        if (!(value - value === 0)) {
          flush();
          if (line.length) lines.push(line);
          line = [];
          column = NaN;
          continue;
        }
        const px = affine ? a + i * b : sx(this.xAt(s, i));
        const py = sy(value);
        const c = Math.floor(px * dpr);
        if (c !== column) {
          flush();
          column = c;
          fx = nx = mx = px;
          fy = ny = my = py;
          fi = ni = mi = i;
          count = 1;
        } else {
          count++;
          if (py > ny) { ny = py; nx = px; ni = i; }
          if (py < my) { my = py; mx = px; mi = i; }
        }
        lx = px;
        ly = py;
        li = i;
      }
      flush();
      if (line.length) lines.push(line);
      return lines;
    }

    /** @param {(x: number) => number} sx @param {(y: number) => number} sy @param {Rect} rect */
    drawLines(sx, sy, rect) {
      const ctx = this.ctx;
      ctx.lineJoin = "round";
      ctx.lineCap = "round";
      for (const s of this.series) {
        if (!s.visible) continue;
        const lines = this.decimate(s, sx, sy);
        const spec = s.spec;
        if (spec.fill) {
          const zero = Math.max(rect.t, Math.min(rect.b, sy(0)));
          /** @type {[string, number, number][]} */
          const halves = [["pos-fill", rect.t, zero], ["neg-fill", zero, rect.b]];
          for (const [role, top, bottom] of halves) {
            ctx.save();
            ctx.beginPath();
            ctx.rect(rect.l, top, rect.r - rect.l, bottom - top);
            ctx.clip();
            ctx.fillStyle = color(role);
            ctx.beginPath();
            for (const line of lines) {
              ctx.moveTo(line[0], zero);
              for (let k = 0; k < line.length; k += 2) ctx.lineTo(line[k], line[k + 1]);
              ctx.lineTo(line[line.length - 2], zero);
              ctx.closePath();
            }
            ctx.fill();
            ctx.restore();
          }
        }
        ctx.globalAlpha = spec.alpha || 1;
        ctx.strokeStyle = color(spec.color);
        ctx.lineWidth = spec.width || 1.5;
        ctx.setLineDash(spec.dash ? [6, 4] : []);
        ctx.beginPath();
        for (const line of lines) {
          ctx.moveTo(line[0], line[1]);
          if (line.length === 2) ctx.lineTo(line[0] + 0.01, line[1]);
          for (let k = 2; k < line.length; k += 2) ctx.lineTo(line[k], line[k + 1]);
        }
        ctx.stroke();
        ctx.setLineDash([]);
        ctx.globalAlpha = 1;
      }
      for (const point of this.spec.points || []) {
        const x = sx(point.x);
        const y = sy(point.y);
        ctx.beginPath();
        ctx.arc(x, y, 3.5, 0, 2 * Math.PI);
        ctx.fillStyle = color(point.color);
        ctx.fill();
        ctx.lineWidth = 1.5;
        ctx.strokeStyle = color("panel");
        ctx.stroke();
      }
    }

    /** @param {(y: number) => number} sy @param {Rect} rect */
    drawBars(sy, rect) {
      const ctx = this.ctx;
      const bars = /** @type {BarSpec[]} */ (this.bars);
      const band = (rect.r - rect.l) / bars.length;
      const width = Math.max(2, Math.min(band * 0.64, 56));
      const zero = sy(0);
      ctx.lineWidth = 1.25;
      bars.forEach((bar, i) => {
        const x = rect.l + band * (i + 0.5) - width / 2;
        const y = sy(bar.value);
        const top = Math.min(y, zero);
        const height = Math.max(1, Math.abs(y - zero));
        ctx.fillStyle = color(bar.value >= 0 ? "pos-fill" : "neg-fill");
        ctx.fillRect(x, top, width, height);
        ctx.strokeStyle = color(bar.value >= 0 ? "left" : "right");
        ctx.strokeRect(x + 0.5, top + 0.5, width - 1, height - 1);
      });
      ctx.strokeStyle = color("zero");
      ctx.beginPath();
      ctx.moveTo(rect.l, Math.round(zero) + 0.5);
      ctx.lineTo(rect.r, Math.round(zero) + 0.5);
      ctx.stroke();
    }

    // ------------------------------------------------------------ hover

    drawHover() {
      const ctx = this.octx;
      ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
      ctx.clearRect(0, 0, this.width, this.height);
      const rect = this.rect;
      const p = this.pointer;
      if (!rect || !p || this.drag || p.x < rect.l || p.x > rect.r || p.y < rect.t || p.y > rect.b) {
        this.tip.hidden = true;
        return;
      }
      const sx = this.xMapper(rect);
      const sy = this.yMapper(rect);
      /** @type {string[]} */
      const rows = [];
      let head = "";
      if (this.bars) {
        const bars = this.bars;
        const band = (rect.r - rect.l) / bars.length;
        const i = Math.floor((p.x - rect.l) / band);
        if (i < 0 || i >= bars.length) {
          this.tip.hidden = true;
          return;
        }
        const bar = bars[i];
        const width = Math.max(2, Math.min(band * 0.64, 56));
        const x = rect.l + band * (i + 0.5);
        const y = sy(bar.value);
        const zero = sy(0);
        ctx.strokeStyle = color("ink");
        ctx.lineWidth = 1.5;
        ctx.strokeRect(x - width / 2, Math.min(y, zero), width, Math.max(1, Math.abs(y - zero)));
        const unit = this.spec.y.unit || "";
        const label = fixed(bar.value, this.spec.y.digits || 1) + (unit ? " " + unit : "");
        ctx.font = "600 11px " + (theme.font || "sans-serif");
        ctx.fillStyle = color("ink");
        ctx.textAlign = "center";
        ctx.textBaseline = bar.value >= 0 ? "bottom" : "top";
        ctx.fillText(label, x, bar.value >= 0 ? Math.max(rect.t + 12, y - 4) : Math.min(rect.b - 12, y + 4));
        head = bar.detail;
        rows.push('<div class="row"><span>' + escapeHtml(this.spec.y.label) + "</span><b>" + escapeHtml(label) + "</b></div>");
      } else {
        const x = this.fromT(this.tx(this.view.x0) + ((p.x - rect.l) / (rect.r - rect.l)) * (this.tx(this.view.x1) - this.tx(this.view.x0)));
        let snapped = NaN;
        for (const s of this.series) {
          if (!s.visible || s.spec.hover === false) continue;
          const i = this.nearest(s, x);
          if (i < 0) continue;
          const sxv = this.xAt(s, i);
          const value = s.ys[i];
          if (isNaN(snapped)) snapped = sxv;
          if (isFinite(value)) {
            ctx.beginPath();
            ctx.arc(sx(sxv), sy(value), 3.5, 0, 2 * Math.PI);
            ctx.fillStyle = color(s.spec.color);
            ctx.fill();
            ctx.lineWidth = 1.5;
            ctx.strokeStyle = color("panel");
            ctx.stroke();
          }
          const unit = this.spec.y.unit ? " " + this.spec.y.unit : "";
          rows.push(
            '<div class="row"><span class="swatch' + (s.spec.dash ? " dashed" : "") + '" style="color:' + color(s.spec.color) + '"></span><span>' +
              escapeHtml(s.spec.name) + "</span><b>" + escapeHtml(readout(value, this.spec.y, 2) + unit) + "</b></div>",
          );
        }
        if (isNaN(snapped)) snapped = x;
        const cx = Math.round(sx(snapped)) + 0.5;
        ctx.strokeStyle = color("muted");
        ctx.lineWidth = 1;
        ctx.setLineDash([3, 3]);
        ctx.beginPath();
        ctx.moveTo(cx, rect.t);
        ctx.lineTo(cx, rect.b);
        ctx.stroke();
        ctx.setLineDash([]);
        const unit = this.spec.x.unit || "";
        head = readout(snapped, this.spec.x, this.logX ? 1 : 2) + (unit ? " " + unit : "");
      }
      if (!rows.length) {
        this.tip.hidden = true;
        return;
      }
      this.tip.innerHTML = '<div class="head">' + escapeHtml(head) + "</div>" + rows.join("");
      this.tip.hidden = false;
      const tw = this.tip.offsetWidth;
      const th = this.tip.offsetHeight;
      let left = p.x + 14;
      if (left + tw > this.width - 2) left = p.x - 14 - tw;
      let top = p.y - th - 10;
      if (top < 0) top = Math.min(p.y + 14, this.height - th);
      this.tip.style.left = Math.max(0, left) + "px";
      this.tip.style.top = Math.max(0, top) + "px";
    }

    // ------------------------------------------------------------ interaction

    /** @param {View} view @param {boolean} [linkX] */
    setView(view, linkX) {
      this.view = view;
      schedule(this);
      if (linkX && this.spec.link) {
        for (const other of links.get(this.spec.link) || []) {
          if (other === this) continue;
          other.view = { x0: view.x0, x1: view.x1, y0: other.view.y0, y1: other.view.y1 };
          schedule(other);
        }
      }
    }

    /** @param {"x" | "y"} axis @param {number} factor @param {number} at pixel */
    zoom(axis, factor, at) {
      const rect = this.rect;
      if (!rect) return;
      const v = this.view;
      if (axis === "x") {
        const t0 = this.tx(v.x0);
        const t1 = this.tx(v.x1);
        const c = t0 + ((at - rect.l) / (rect.r - rect.l)) * (t1 - t0);
        const [n0, n1] = this.clampX(c + (t0 - c) * factor, c + (t1 - c) * factor);
        if (!(n1 - n0 > 1e-9 * Math.max(1, Math.abs(c)))) return;
        this.setView({ x0: this.fromT(n0), x1: this.fromT(n1), y0: v.y0, y1: v.y1 }, true);
      } else {
        const c = v.y1 - ((at - rect.t) / (rect.b - rect.t)) * (v.y1 - v.y0);
        const n0 = c + (v.y0 - c) * factor;
        const n1 = c + (v.y1 - c) * factor;
        if (!(n1 - n0 > 1e-12 * Math.max(1, Math.abs(c)))) return;
        this.setView({ x0: v.x0, x1: v.x1, y0: n0, y1: n1 }, false);
      }
    }

    /** @param {number} dx pixels @param {number} dy pixels @param {View} from */
    pan(dx, dy, from) {
      const rect = this.rect;
      if (!rect) return;
      const t0 = this.tx(from.x0);
      const t1 = this.tx(from.x1);
      const dt = (dx / (rect.r - rect.l)) * (t1 - t0);
      const dv = (dy / (rect.b - rect.t)) * (from.y1 - from.y0);
      const [n0, n1] = this.clampX(t0 - dt, t1 - dt);
      this.setView({ x0: this.fromT(n0), x1: this.fromT(n1), y0: from.y0 + dv, y1: from.y1 + dv }, dx !== 0);
    }

    reset() {
      this.setView(Object.assign({}, this.home), true);
    }

    listen() {
      const plot = this.plot;
      /** @param {PointerEvent | WheelEvent | MouseEvent} e */
      const local = (e) => {
        const box = plot.getBoundingClientRect();
        return { x: e.clientX - box.left, y: e.clientY - box.top };
      };
      const inside = (/** @type {{x: number, y: number}} */ p) =>
        !!this.rect && p.x >= this.rect.l && p.x <= this.rect.r && p.y >= this.rect.t && p.y <= this.rect.b;
      plot.addEventListener("pointermove", (e) => {
        this.pointer = local(e);
        if (this.drag) {
          this.pan(this.pointer.x - this.drag.x, this.pointer.y - this.drag.y, this.drag.view);
          return;
        }
        hoverDirty.add(this);
        request();
      });
      plot.addEventListener("pointerleave", () => {
        this.pointer = null;
        hoverDirty.add(this);
        request();
      });
      if (this.bars) return;
      plot.addEventListener("pointerdown", (e) => {
        const p = local(e);
        if (e.button !== 0 || !inside(p)) return;
        this.drag = { x: p.x, y: p.y, view: Object.assign({}, this.view) };
        plot.setPointerCapture(e.pointerId);
        plot.classList.add("dragging");
        hoverDirty.add(this);
        request();
      });
      const release = () => {
        if (!this.drag) return;
        this.drag = null;
        plot.classList.remove("dragging");
        hoverDirty.add(this);
        request();
      };
      plot.addEventListener("pointerup", release);
      plot.addEventListener("pointercancel", release);
      plot.addEventListener(
        "wheel",
        (e) => {
          const p = local(e);
          if (!inside(p)) return;
          e.preventDefault();
          const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 240 : 1;
          const delta = Math.max(-400, Math.min(400, (e.deltaY || e.deltaX) * unit));
          const factor = Math.exp(delta * 0.0015);
          if (e.shiftKey || e.ctrlKey) this.zoom("y", factor, p.y);
          else this.zoom("x", factor, p.x);
        },
        { passive: false },
      );
      plot.addEventListener("dblclick", (e) => {
        if (inside(local(e))) this.reset();
      });
      plot.addEventListener("keydown", (e) => {
        const rect = this.rect;
        if (!rect) return;
        const cx = (rect.l + rect.r) / 2;
        const cy = (rect.t + rect.b) / 2;
        const w = rect.r - rect.l;
        const h = rect.b - rect.t;
        const axis = e.shiftKey ? "y" : "x";
        let handled = true;
        if (e.key === "ArrowLeft") this.pan(w * 0.1, 0, this.view);
        else if (e.key === "ArrowRight") this.pan(-w * 0.1, 0, this.view);
        else if (e.key === "ArrowUp") this.pan(0, h * 0.1, this.view);
        else if (e.key === "ArrowDown") this.pan(0, -h * 0.1, this.view);
        else if (e.key === "+" || e.key === "=") this.zoom(axis, 0.8, axis === "x" ? cx : cy);
        else if (e.key === "-" || e.key === "_") this.zoom(axis, 1.25, axis === "x" ? cx : cy);
        else if (e.key === "0" || e.key === "Home") this.reset();
        else handled = false;
        if (handled) e.preventDefault();
      });
    }
  }

  /** @param {Float32Array} xs @param {number} x first index with xs[i] >= x */
  function lowerBound(xs, x) {
    let lo = 0;
    let hi = xs.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (xs[mid] < x) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }

  /** @param {number} lo @param {number} hi @param {number} fraction */
  function padded(lo, hi, fraction) {
    if (!isFinite(lo) || !isFinite(hi)) return [-1, 1];
    if (hi - lo < 1e-12) {
      const d = Math.max(Math.abs(hi) * 0.1, 1e-3);
      return [lo - d, hi + d];
    }
    const pad = (hi - lo) * fraction;
    return [lo - pad, hi + pad];
  }

  // ---------------------------------------------------------------- page

  function build() {
    const page = el("div", "page");
    const header = el("header", "top");
    header.appendChild(el("h1", "", manifest.title));
    if (manifest.subtitle) header.appendChild(el("p", "", manifest.subtitle));
    page.appendChild(header);
    const tabs = manifest.tabs || [];
    const nav = el("nav", "tabs");
    nav.setAttribute("role", "tablist");
    nav.setAttribute("aria-label", "Analysis");
    if (tabs.length > 1) page.appendChild(nav);
    const main = el("main");
    page.appendChild(main);
    const footer = el("footer");
    footer.innerHTML =
      "Drag to pan · <kbd>wheel</kbd> zooms time or frequency, <kbd>Shift</kbd>/<kbd>Ctrl</kbd> + <kbd>wheel</kbd> zooms the level axis · double-click resets · legend entries toggle curves · focused charts take arrows, <kbd>+</kbd>/<kbd>−</kbd> and <kbd>0</kbd>. " +
      escapeHtml(manifest.generator || "");
    page.appendChild(footer);
    document.body.appendChild(page);

    /** @type {{spec: TabSpec, button: HTMLButtonElement, panel: HTMLElement, built: boolean}[]} */
    const entries = tabs.map((spec, index) => {
      const button = el("button", "", spec.title);
      button.type = "button";
      button.id = "tab-" + spec.id;
      button.setAttribute("role", "tab");
      button.setAttribute("aria-controls", "panel-" + spec.id);
      button.tabIndex = -1;
      nav.appendChild(button);
      const panel = el("section");
      panel.id = "panel-" + spec.id;
      panel.setAttribute("role", "tabpanel");
      panel.setAttribute("aria-labelledby", button.id);
      panel.hidden = true;
      main.appendChild(panel);
      button.addEventListener("click", () => activate(index, true));
      button.addEventListener("keydown", (e) => {
        const next = e.key === "ArrowRight" ? index + 1 : e.key === "ArrowLeft" ? index - 1 : e.key === "Home" ? 0 : e.key === "End" ? tabs.length - 1 : -1;
        if (next < 0 && e.key !== "ArrowLeft") return;
        e.preventDefault();
        const target = (next + tabs.length) % tabs.length;
        activate(target, true);
        entries[target].button.focus();
      });
      return { spec: spec, button: button, panel: panel, built: false };
    });

    /** @param {number} index @param {boolean} remember */
    function activate(index, remember) {
      entries.forEach((entry, i) => {
        const active = i === index;
        entry.button.setAttribute("aria-selected", String(active));
        entry.button.tabIndex = active ? 0 : -1;
        if (active && !entry.built) {
          entry.built = true;
          const data = loadBlob(entry.spec.blob);
          if (entry.spec.note) entry.panel.appendChild(el("p", "note", entry.spec.note));
          const grid = el("div", "grid");
          entry.panel.appendChild(grid);
          for (const chart of entry.spec.charts) new Chart(chart, data, grid);
        }
        entry.panel.hidden = !active;
      });
      if (remember && tabs[index]) {
        try {
          history.replaceState(null, "", "#" + tabs[index].id);
        } catch (_error) {
          /* file:// documents may refuse history updates */
        }
      }
    }

    const wanted = location.hash.slice(1);
    const initial = Math.max(0, tabs.findIndex((t) => t.id === wanted));
    if (tabs.length) activate(initial, false);
    /** @type {any} */ (window).impulciferReport.activate = (/** @type {number} */ i) => activate(i, false);
  }

  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    readTheme();
    for (const chart of charts.values()) {
      chart.paintSwatches();
      schedule(chart);
    }
  });

  // Off-screen charts are skipped while browsing; printing needs them all.
  window.addEventListener("beforeprint", () => {
    for (const chart of charts.values()) if (chart.width > 0 && chart.needsDraw) chart.draw();
  });

  build();
  /** @type {any} */ (window).impulciferReport.charts = charts;
})();
