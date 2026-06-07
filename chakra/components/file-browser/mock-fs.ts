// components/file-browser/mock-fs.ts - Portable mock filesystem and helpers for the file browser shell.

export type BrowserNodeKind = "volume" | "directory" | "file";
export type EmbeddedStatus = "ready" | "convert" | "unsupported";
export type SortKey = "name" | "kind" | "size" | "modified";

export interface BrowserNode {
  id: string;
  name: string;
  path: string;
  kind: BrowserNodeKind;
  fileType: string;
  modifiedAt: string;
  note: string;
  preview: string;
  embeddedStatus: EmbeddedStatus;
  embeddedFormats: string[];
  sizeBytes?: number;
  children?: BrowserNode[];
}

export interface BrowserTreeNode {
  id: string;
  name: string;
  path: string;
  type: "root" | "volume" | "directory";
  children?: BrowserTreeNode[];
}

export const INITIAL_DIRECTORY_PATH = "/sd-card/projects/flight-deck/assets";
export const INITIAL_SELECTED_PATH =
  "/sd-card/projects/flight-deck/assets/splash-boot.rle";

export const MOCK_FILESYSTEM: BrowserNode[] = [
  {
    id: "sd-card",
    name: "SD Card",
    path: "/sd-card",
    kind: "volume",
    fileType: "volume",
    modifiedAt: "2026-04-08 17:20",
    note: "Removable media mounted over SDMMC1. Best target for demos and captures.",
    preview: "Volume ready. 4.2 GB free. FAT assets plus raw captures.",
    embeddedStatus: "ready",
    embeddedFormats: ["rle", "raw", "wav"],
    children: [
      {
        id: "sd-card/projects",
        name: "projects",
        path: "/sd-card/projects",
        kind: "directory",
        fileType: "folder",
        modifiedAt: "2026-04-08 17:14",
        note: "Project workspaces staged for the 747I shell.",
        preview: "Contains flight deck, icon, and splash worktrees.",
        embeddedStatus: "ready",
        embeddedFormats: ["raw", "rle", "wav", "json"],
        children: [
          {
            id: "sd-card/projects/flight-deck",
            name: "flight-deck",
            path: "/sd-card/projects/flight-deck",
            kind: "directory",
            fileType: "folder",
            modifiedAt: "2026-04-08 16:59",
            note: "Primary browser prototype workspace mirroring the board demo.",
            preview: "Three-pane browser shell with asset conversion outputs.",
            embeddedStatus: "ready",
            embeddedFormats: ["raw", "rle", "wav", "json"],
            children: [
              {
                id: "sd-card/projects/flight-deck/assets",
                name: "assets",
                path: "/sd-card/projects/flight-deck/assets",
                kind: "directory",
                fileType: "folder",
                modifiedAt: "2026-04-08 16:42",
                note: "Embedded-safe assets and conversion outputs.",
                preview: "Portable assets only. Good place to point the demo browser by default.",
                embeddedStatus: "ready",
                embeddedFormats: ["raw", "rle", "wav"],
                children: [
                  {
                    id: "splash-boot",
                    name: "splash-boot.rle",
                    path: "/sd-card/projects/flight-deck/assets/splash-boot.rle",
                    kind: "file",
                    fileType: "RLE image",
                    modifiedAt: "2026-04-08 16:30",
                    sizeBytes: 123904,
                    note: "Compressed splash image already sized for the STM32H747I-DISCO panel.",
                    preview:
                      "RLE container\nsize: 800x480\npalette: RGB565\nframes: 1\nstatus: ready for flash-backed assets",
                    embeddedStatus: "ready",
                    embeddedFormats: ["rle", "raw"],
                  },
                  {
                    id: "wing-icons",
                    name: "wing-icons.raw",
                    path: "/sd-card/projects/flight-deck/assets/wing-icons.raw",
                    kind: "file",
                    fileType: "RAW bitmap",
                    modifiedAt: "2026-04-08 16:28",
                    sizeBytes: 36864,
                    note: "Decoded icon strip and wing sheet for on-device composition.",
                    preview:
                      "RAW bitmap atlas\nformat: ARGB8888\ntiles: settings, files, info, cpu, audio",
                    embeddedStatus: "ready",
                    embeddedFormats: ["raw"],
                  },
                  {
                    id: "engine-loop",
                    name: "engine-loop.wav",
                    path: "/sd-card/projects/flight-deck/assets/engine-loop.wav",
                    kind: "file",
                    fileType: "WAV audio",
                    modifiedAt: "2026-04-08 16:11",
                    sizeBytes: 2831152,
                    note: "PCM preview audio for the audio scope and event pipeline.",
                    preview:
                      "WAV\nchannels: 1\nsample rate: 22050 Hz\nlength: 64.1 s\nready for SD-backed playback",
                    embeddedStatus: "ready",
                    embeddedFormats: ["wav"],
                  },
                  {
                    id: "panel-layout",
                    name: "panel-layout.json",
                    path: "/sd-card/projects/flight-deck/assets/panel-layout.json",
                    kind: "file",
                    fileType: "JSON layout",
                    modifiedAt: "2026-04-08 15:54",
                    sizeBytes: 18432,
                    note: "Screen composition spec for the demo shell.",
                    preview:
                      "{\n  \"shell\": \"file-browser-frame\",\n  \"panes\": [\"tree\", \"table\", \"details\"],\n  \"embeddedFallback\": \"single-pane\"\n}",
                    embeddedStatus: "convert",
                    embeddedFormats: ["json"],
                  },
                ],
              },
              {
                id: "sd-card/projects/flight-deck/captures",
                name: "captures",
                path: "/sd-card/projects/flight-deck/captures",
                kind: "directory",
                fileType: "folder",
                modifiedAt: "2026-04-08 16:01",
                note: "Reference captures and imports from desktop tooling.",
                preview: "Mostly useful for conversion and preview, not direct embedded use.",
                embeddedStatus: "convert",
                embeddedFormats: ["png", "wav"],
                children: [
                  {
                    id: "flight-panel-png",
                    name: "747-panel-reference.png",
                    path: "/sd-card/projects/flight-deck/captures/747-panel-reference.png",
                    kind: "file",
                    fileType: "PNG image",
                    modifiedAt: "2026-04-08 15:50",
                    sizeBytes: 894221,
                    note: "Browser reference capture from the desktop prototype.",
                    preview:
                      "PNG capture\nsource: simulator export\nnext step: convert to RAW or RLE for embedded playback",
                    embeddedStatus: "convert",
                    embeddedFormats: ["png", "raw", "rle"],
                  },
                  {
                    id: "scope-ref",
                    name: "audio-scope-reference.wav",
                    path: "/sd-card/projects/flight-deck/captures/audio-scope-reference.wav",
                    kind: "file",
                    fileType: "WAV audio",
                    modifiedAt: "2026-04-08 15:47",
                    sizeBytes: 512887,
                    note: "Reference capture for audio scope alignment.",
                    preview:
                      "WAV capture\nsource: desktop recorder\nstatus: playable after staging into demo assets",
                    embeddedStatus: "ready",
                    embeddedFormats: ["wav"],
                  },
                ],
              },
            ],
          },
        ],
      },
      {
        id: "sd-card/recents",
        name: "recents",
        path: "/sd-card/recents",
        kind: "directory",
        fileType: "folder",
        modifiedAt: "2026-04-07 19:12",
        note: "Operator shortcuts and most recent selections.",
        preview: "Recent files and staging manifests for rapid file-open flows.",
        embeddedStatus: "ready",
        embeddedFormats: ["json", "wav", "rle"],
        children: [
          {
            id: "recent-manifest",
            name: "open-history.json",
            path: "/sd-card/recents/open-history.json",
            kind: "file",
            fileType: "JSON log",
            modifiedAt: "2026-04-07 19:08",
            sizeBytes: 2901,
            note: "Selection history for projected quick-open support.",
            preview:
              "{\n  \"recent\": [\"splash-boot.rle\", \"engine-loop.wav\", \"panel-layout.json\"]\n}",
            embeddedStatus: "convert",
            embeddedFormats: ["json"],
          },
        ],
      },
    ],
  },
  {
    id: "qspi-flash",
    name: "QSPI Flash",
    path: "/qspi-flash",
    kind: "volume",
    fileType: "volume",
    modifiedAt: "2026-04-06 11:10",
    note: "Non-removable storage with tighter space and stricter format rules.",
    preview: "Faster reads than SD. Good for splash, icons, and layout manifests.",
    embeddedStatus: "ready",
    embeddedFormats: ["rle", "raw", "json"],
    children: [
      {
        id: "qspi-flash/ui-theme",
        name: "ui-theme",
        path: "/qspi-flash/ui-theme",
        kind: "directory",
        fileType: "folder",
        modifiedAt: "2026-04-06 10:58",
        note: "Theme assets generated from Chakra tokens and embedded-safe palettes.",
        preview: "Use this to verify token mappings and conversion outputs.",
        embeddedStatus: "ready",
        embeddedFormats: ["yaml", "json", "raw"],
        children: [
          {
            id: "token-yaml",
            name: "tokens.yaml",
            path: "/qspi-flash/ui-theme/tokens.yaml",
            kind: "file",
            fileType: "Theme tokens",
            modifiedAt: "2026-04-06 10:55",
            sizeBytes: 6124,
            note: "Generated from Chakra semantic tokens via rlvgl-creator.",
            preview:
              "colors:\n  primary: \"#f4b842\"\n  background: \"#08131d\"\nspacing:\n  md: 8",
            embeddedStatus: "ready",
            embeddedFormats: ["yaml", "json"],
          },
        ],
      },
    ],
  },
  {
    id: "workspace",
    name: "Workspace",
    path: "/workspace",
    kind: "volume",
    fileType: "volume",
    modifiedAt: "2026-04-09 09:35",
    note: "Repository-facing source tree. Useful for import and conversion, not for direct flash.",
    preview: "Source-of-truth code and docs. Treat as reference storage in the browser shell.",
    embeddedStatus: "convert",
    embeddedFormats: ["rs", "md", "json"],
    children: [
      {
        id: "workspace/examples",
        name: "examples",
        path: "/workspace/examples",
        kind: "directory",
        fileType: "folder",
        modifiedAt: "2026-04-09 09:28",
        note: "Board demos and simulator fixtures.",
        preview: "Contains the STM32H747I-DISCO demo the browser shell mirrors.",
        embeddedStatus: "convert",
        embeddedFormats: ["rs", "md"],
        children: [
          {
            id: "workspace/examples/stm32h747i-disco",
            name: "stm32h747i-disco",
            path: "/workspace/examples/stm32h747i-disco",
            kind: "directory",
            fileType: "folder",
            modifiedAt: "2026-04-09 09:22",
            note: "Current embedded demo source tree.",
            preview: "Source workspace only. Ideal for traceability, not for direct on-device browsing.",
            embeddedStatus: "convert",
            embeddedFormats: ["rs", "md", "rle", "raw"],
            children: [
              {
                id: "workspace/examples/stm32h747i-disco/README",
                name: "README.md",
                path: "/workspace/examples/stm32h747i-disco/README.md",
                kind: "file",
                fileType: "Markdown",
                modifiedAt: "2026-04-09 09:19",
                sizeBytes: 5232,
                note: "Human-facing demo overview. Good preview source, not an embedded asset.",
                preview:
                  "# STM32H747I-DISCO Demo\nDemonstrates rlvgl on the discovery board...",
                embeddedStatus: "unsupported",
                embeddedFormats: ["md"],
              },
            ],
          },
        ],
      },
    ],
  },
];

export function buildTreeNodes(nodes: BrowserNode[]): BrowserTreeNode[] {
  return nodes.map((node) => ({
    id: node.id,
    name: node.name,
    path: node.path,
    type: node.kind === "volume" ? "volume" : "directory",
    children: buildTreeNodes((node.children ?? []).filter((child) => child.kind !== "file")),
  }));
}

export function findNodeByPath(
  nodes: BrowserNode[],
  path: string,
): BrowserNode | undefined {
  for (const node of nodes) {
    if (node.path === path) {
      return node;
    }

    if (node.children) {
      const match = findNodeByPath(node.children, path);
      if (match) {
        return match;
      }
    }
  }

  return undefined;
}

export function getPathChain(nodes: BrowserNode[], path: string): BrowserNode[] {
  for (const node of nodes) {
    if (node.path === path) {
      return [node];
    }

    if (node.children) {
      const chain = getPathChain(node.children, path);
      if (chain.length > 0) {
        return [node, ...chain];
      }
    }
  }

  return [];
}

export function listDirectoryEntries(nodes: BrowserNode[], path: string): BrowserNode[] {
  if (path === "/") {
    return nodes;
  }

  return findNodeByPath(nodes, path)?.children ?? [];
}

export function mergeUniquePaths(paths: string[], nextPaths: string[]): string[] {
  const merged = new Set(paths);
  for (const path of nextPaths) {
    merged.add(path);
  }

  return Array.from(merged);
}

export function formatBytes(sizeBytes?: number): string {
  if (sizeBytes === undefined) {
    return "—";
  }

  if (sizeBytes < 1024) {
    return `${sizeBytes} B`;
  }

  if (sizeBytes < 1024 * 1024) {
    return `${(sizeBytes / 1024).toFixed(1)} KB`;
  }

  return `${(sizeBytes / (1024 * 1024)).toFixed(2)} MB`;
}
