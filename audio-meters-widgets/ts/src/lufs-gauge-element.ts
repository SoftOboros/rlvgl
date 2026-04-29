// `<rlvgl-lufs-gauge>` custom element. Wraps LufsGaugeCore with a
// <canvas> + rAF loop. Browser-only.

import {
  LufsGaugeCore,
  type LufsGaugeConfig,
  type LufsSink,
} from "./lufs-gauge-core.ts";
import type { Scale, Skin } from "./skin.ts";

declare const HTMLElement: { new (): RlvglLufsGaugeElement };
declare const customElements: {
  define(name: string, ctor: { new (): RlvglLufsGaugeElement }): void;
  get(name: string): unknown;
};

interface BrowserCanvasContext {
  fillStyle: string;
  font: string;
  textBaseline: string;
  fillRect(x: number, y: number, w: number, h: number): void;
  fillText(text: string, x: number, y: number): void;
}

interface BrowserCanvasElement {
  width: number;
  height: number;
  getContext(name: "2d"): BrowserCanvasContext | null;
}

interface BrowserShadowRoot {
  appendChild<T>(child: T): T;
}

const DEFAULT_FONT = "16px ui-monospace, SFMono-Regular, Menlo, monospace";

export class RlvglLufsGaugeElement extends HTMLElement {
  private core: LufsGaugeCore | null = null;
  private canvas: BrowserCanvasElement | null = null;
  private ctx: BrowserCanvasContext | null = null;
  private rafId: number | null = null;
  private lastTimestamp: number | null = null;
  private latestDbfs: number = -120;

  static get observedAttributes(): string[] {
    return ["src-skin", "src-scale", "width", "height", "font"];
  }

  connectedCallback(): void {
    const shadow = (this as unknown as {
      attachShadow(opts: { mode: string }): BrowserShadowRoot;
    }).attachShadow({ mode: "open" });
    const document = (globalThis as unknown as {
      document: { createElement(tag: string): BrowserCanvasElement };
    }).document;
    const canvas = document.createElement("canvas") as BrowserCanvasElement;
    canvas.width = this.attrNum("width", 280);
    canvas.height = this.attrNum("height", 140);
    shadow.appendChild(canvas);
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
    if (this.ctx) {
      this.ctx.font = this.attrStr("font", DEFAULT_FONT);
      this.ctx.textBaseline = "alphabetic";
    }
    void this.loadAndStart();
  }

  disconnectedCallback(): void {
    if (this.rafId !== null) {
      const cancel = (globalThis as unknown as {
        cancelAnimationFrame(id: number): void;
      }).cancelAnimationFrame;
      cancel(this.rafId);
      this.rafId = null;
    }
  }

  feed(dbfs: number): void {
    this.latestDbfs = dbfs;
  }

  integrated(): number {
    return this.core?.integratedDbValue() ?? -120;
  }
  shortTerm(): number {
    return this.core?.shortTermDbValue() ?? -120;
  }
  momentary(): number {
    return this.core?.momentaryDbValue() ?? -120;
  }

  private attrNum(name: string, fallback: number): number {
    const v = (this as unknown as { getAttribute(n: string): string | null })
      .getAttribute(name);
    if (v === null) return fallback;
    const n = Number(v);
    return Number.isFinite(n) ? n : fallback;
  }
  private attrStr(name: string, fallback: string): string {
    const v = (this as unknown as { getAttribute(n: string): string | null })
      .getAttribute(name);
    return v ?? fallback;
  }

  private async loadAndStart(): Promise<void> {
    const fetch = (globalThis as unknown as { fetch: typeof globalThis.fetch }).fetch;
    const scaleSrc =
      (this as unknown as { getAttribute(n: string): string | null }).getAttribute("src-scale") ?? "";
    const skinSrc =
      (this as unknown as { getAttribute(n: string): string | null }).getAttribute("src-skin") ?? "";
    if (!scaleSrc || !skinSrc) return;
    const [sr, kr] = await Promise.all([fetch(scaleSrc), fetch(skinSrc)]);
    const scale = (await sr.json()) as Scale;
    const skin = (await kr.json()) as Skin;
    const cfg: LufsGaugeConfig = { scale, skin };
    this.core = new LufsGaugeCore(cfg);
    this.startLoop();
  }

  private startLoop(): void {
    const raf = (globalThis as unknown as {
      requestAnimationFrame(cb: (t: number) => void): number;
    }).requestAnimationFrame;
    const tick = (timestamp: number) => {
      const last = this.lastTimestamp;
      this.lastTimestamp = timestamp;
      const dt = last === null ? 1 / 60 : Math.max(0, (timestamp - last) / 1000);
      this.core?.update(this.latestDbfs, dt);
      this.paint();
      this.rafId = raf(tick);
    };
    this.rafId = raf(tick);
  }

  private paint(): void {
    const core = this.core;
    const canvas = this.canvas;
    const ctx = this.ctx;
    if (!core || !canvas || !ctx) return;
    const sink: LufsSink = {
      fillRect(x, y, w, h, color) {
        ctx.fillStyle = color;
        ctx.fillRect(x, y, w, h);
      },
      drawText(x, y, text, color) {
        ctx.fillStyle = color;
        ctx.fillText(text, x, y);
      },
    };
    core.draw(sink, 0, 0, canvas.width, canvas.height);
  }
}

export function defineRlvglLufsGauge(name: string = "rlvgl-lufs-gauge"): void {
  if (typeof customElements === "undefined") return;
  if (customElements.get(name)) return;
  customElements.define(name, RlvglLufsGaugeElement as unknown as {
    new (): RlvglLufsGaugeElement;
  });
}
