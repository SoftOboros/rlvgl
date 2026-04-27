// `<rlvgl-needle-vu>` custom element. Wraps NeedleVuCore with a
// <canvas> and a per-frame requestAnimationFrame loop. Browser-only.
//
//   <rlvgl-needle-vu
//       src-skin="/assets/audio-meters/skins/broadcast_classic_needle.json"
//       src-scale="/assets/audio-meters/scales/vu_broadcast.json"
//       width="320" height="200">
//   </rlvgl-needle-vu>

import { NeedleVuCore, type NeedleSink, type NeedleVuConfig } from "./needle-vu-core.ts";
import type { Scale, Skin } from "./skin.ts";

declare const HTMLElement: { new (): RlvglNeedleVuElement };
declare const customElements: {
  define(name: string, ctor: { new (): RlvglNeedleVuElement }): void;
  get(name: string): unknown;
};

interface BrowserCanvasContext {
  fillStyle: string;
  fillRect(x: number, y: number, w: number, h: number): void;
}

interface BrowserCanvasElement {
  width: number;
  height: number;
  getContext(name: "2d"): BrowserCanvasContext | null;
}

interface BrowserShadowRoot {
  appendChild<T>(child: T): T;
}

export class RlvglNeedleVuElement extends HTMLElement {
  private core: NeedleVuCore | null = null;
  private canvas: BrowserCanvasElement | null = null;
  private ctx: BrowserCanvasContext | null = null;
  private rafId: number | null = null;
  private lastTimestamp: number | null = null;
  private latestDbfs: number = -120;

  static get observedAttributes(): string[] {
    return ["src-skin", "src-scale", "width", "height"];
  }

  connectedCallback(): void {
    const shadow = (this as unknown as {
      attachShadow(opts: { mode: string }): BrowserShadowRoot;
    }).attachShadow({ mode: "open" });
    const document = (globalThis as unknown as {
      document: { createElement(tag: string): BrowserCanvasElement };
    }).document;
    const canvas = document.createElement("canvas") as BrowserCanvasElement;
    canvas.width = this.attrNum("width", 320);
    canvas.height = this.attrNum("height", 200);
    shadow.appendChild(canvas);
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
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

  reading(): number {
    return this.core?.readingDbValue() ?? -120;
  }

  private attrNum(name: string, fallback: number): number {
    const v = (this as unknown as { getAttribute(n: string): string | null })
      .getAttribute(name);
    if (v === null) return fallback;
    const n = Number(v);
    return Number.isFinite(n) ? n : fallback;
  }

  private async loadAndStart(): Promise<void> {
    const fetch = (globalThis as unknown as { fetch: typeof globalThis.fetch }).fetch;
    const scaleSrc =
      (this as unknown as { getAttribute(n: string): string | null }).getAttribute("src-scale") ?? "";
    const skinSrc =
      (this as unknown as { getAttribute(n: string): string | null }).getAttribute("src-skin") ?? "";
    if (!scaleSrc || !skinSrc) return;

    const [scaleResp, skinResp] = await Promise.all([fetch(scaleSrc), fetch(skinSrc)]);
    const scale = (await scaleResp.json()) as Scale;
    const skin = (await skinResp.json()) as Skin;
    const cfg: NeedleVuConfig = { scale, skin };
    this.core = new NeedleVuCore(cfg);
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
    const sink: NeedleSink = {
      fillRect(x, y, w, h, color) {
        ctx.fillStyle = color;
        ctx.fillRect(x, y, w, h);
      },
    };
    core.draw(sink, 0, 0, canvas.width, canvas.height);
  }
}

/** Register the element. Browser-only. */
export function defineRlvglNeedleVu(name: string = "rlvgl-needle-vu"): void {
  if (typeof customElements === "undefined") return;
  if (customElements.get(name)) return;
  customElements.define(name, RlvglNeedleVuElement as unknown as {
    new (): RlvglNeedleVuElement;
  });
}
