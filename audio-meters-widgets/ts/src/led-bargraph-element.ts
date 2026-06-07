// `<rlvgl-led-bargraph>` custom element. Wraps LedBargraphCore with a
// `<canvas>` and a per-frame requestAnimationFrame loop. Browser-only —
// importing this file at module top-level in a Node test will fail
// unless DOM globals are stubbed.
//
// Usage:
//
//   <rlvgl-led-bargraph
//       src-skin="/assets/audio-meters/skins/broadcast_classic_bargraph.json"
//       src-scale="/assets/audio-meters/scales/vu_broadcast.json"
//       width="64"
//       height="320">
//   </rlvgl-led-bargraph>
//
//   const meter = document.querySelector("rlvgl-led-bargraph") as RlvglLedBargraphElement;
//   meter.feed(-20.0);  // pushes one frame at the element's rAF cadence

import {
  LedBargraphCore,
  type DrawSink,
  type LedBargraphConfig,
} from "./led-bargraph-core.ts";
import type { Scale, Skin } from "./skin.ts";

declare const HTMLElement: { new (): RlvglLedBargraphElement };
declare const customElements: {
  define(name: string, ctor: { new (): RlvglLedBargraphElement }): void;
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

/**
 * Custom element. Loads the bound scale + skin JSON via fetch, then
 * runs a rAF loop calling LedBargraphCore.update + draw. Application
 * code feeds new dBFS values via `feed()`; the element interpolates
 * between feeds by calling update with the most recent value at every
 * frame (so the ballistic decays correctly even if the audio thread
 * stops posting).
 */
export class RlvglLedBargraphElement extends HTMLElement {
  private core: LedBargraphCore | null = null;
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
    canvas.width = this.attrNum("width", 64);
    canvas.height = this.attrNum("height", 320);
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

  /** Push the latest per-frame dBFS sample. */
  feed(dbfs: number): void {
    this.latestDbfs = dbfs;
  }

  /** Read back the last ballistic reading (dBFS). */
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

    const [scaleResp, skinResp] = await Promise.all([
      fetch(scaleSrc),
      fetch(skinSrc),
    ]);
    const scale = (await scaleResp.json()) as Scale;
    const skin = (await skinResp.json()) as Skin;

    const cfg: LedBargraphConfig = { scale, skin };
    this.core = new LedBargraphCore(cfg);
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
    const sink: DrawSink = {
      fillRect(x, y, w, h, color) {
        ctx.fillStyle = color;
        ctx.fillRect(x, y, w, h);
      },
    };
    core.draw(sink, 0, 0, canvas.width, canvas.height);
  }
}

/** Register the element. Browser-only — call at module load time. */
export function defineRlvglLedBargraph(
  name: string = "rlvgl-led-bargraph",
): void {
  if (typeof customElements === "undefined") return;
  if (customElements.get(name)) return;
  customElements.define(name, RlvglLedBargraphElement as unknown as {
    new (): RlvglLedBargraphElement;
  });
}
