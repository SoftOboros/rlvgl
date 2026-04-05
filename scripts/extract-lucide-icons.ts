#!/usr/bin/env npx tsx
/**
 * Extract Lucide icon SVG files from lucide-react's ESM distribution.
 *
 * Parses the __iconNode arrays from JS modules and reconstructs standalone
 * SVG files suitable for import into the rlvgl creator asset pipeline.
 *
 * Usage:
 *   npx tsx scripts/extract-lucide-icons.ts [options]
 *
 * Options:
 *   --lucide-path <path>  Path to lucide-react ESM icons dir
 *   --out <dir>           Output directory (default: assets/icons)
 *   --stroke <color>      Stroke color (default: #FFFFFF)
 *   --size <px>           Icon size (default: 24)
 *   --icons <list>        Comma-separated icon names (default: all five)
 */

import { readFileSync, writeFileSync, mkdirSync, existsSync } from "fs";
import { join, resolve, dirname } from "path";

// Default icon set: name -> lucide JS module filename
const DEFAULT_ICONS: Record<string, string> = {
  settings: "settings.js", // gear/config
  file: "file.js", // file
  trash: "trash-2.js", // trash can
  "folder-open": "folder-open.js", // open
  close: "x.js", // close/X
};

// camelCase SVG attributes that need kebab-case conversion
const CAMEL_TO_KEBAB: Record<string, string> = {
  strokeWidth: "stroke-width",
  strokeLinecap: "stroke-linecap",
  strokeLinejoin: "stroke-linejoin",
  fillRule: "fill-rule",
  clipRule: "clip-rule",
  fillOpacity: "fill-opacity",
  strokeOpacity: "stroke-opacity",
  strokeDasharray: "stroke-dasharray",
  strokeDashoffset: "stroke-dashoffset",
  strokeMiterlimit: "stroke-miterlimit",
};

// Attributes to drop (React-specific, not valid SVG)
const DROP_ATTRS = new Set(["key"]);

interface ParsedElement {
  tag: string;
  attrs: Record<string, string>;
}

function parseIconNode(jsSource: string): ParsedElement[] {
  // Extract the array assigned to __iconNode
  const match = jsSource.match(
    /const\s+__iconNode\s*=\s*(\[[\s\S]*?\]);\s*\n/
  );
  if (!match) {
    throw new Error("Could not find __iconNode in source");
  }

  // The array content is valid JSON (arrays of [string, object] pairs)
  let arrayStr = match[1];

  // Lucide uses unquoted keys in some cases; normalize to valid JSON
  // Actually the ESM dist uses proper JS object literal syntax which is
  // close to JSON but keys aren't quoted. Fix that:
  arrayStr = arrayStr.replace(
    /(\{|,)\s*([a-zA-Z_]\w*)\s*:/g,
    '$1 "$2":'
  );

  const nodes: Array<[string, Record<string, string>]> = JSON.parse(arrayStr);
  return nodes.map(([tag, attrs]) => ({ tag, attrs }));
}

function attrsToSvg(attrs: Record<string, string>): string {
  return Object.entries(attrs)
    .filter(([k]) => !DROP_ATTRS.has(k))
    .map(([k, v]) => {
      const svgKey = CAMEL_TO_KEBAB[k] || k;
      return `${svgKey}="${v}"`;
    })
    .join(" ");
}

function buildSvg(
  elements: ParsedElement[],
  stroke: string,
  size: number
): string {
  const children = elements
    .map((el) => `  <${el.tag} ${attrsToSvg(el.attrs)} />`)
    .join("\n");

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="${stroke}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
${children}
</svg>
`;
}

function parseArgs(argv: string[]) {
  const scriptDir = dirname(resolve(argv[1] || __filename));
  const projectRoot = resolve(scriptDir, "..");

  const args = {
    lucidePath: resolve(
      projectRoot,
      "../softoboros/frontend/node_modules/lucide-react/dist/esm/icons"
    ),
    out: resolve(projectRoot, "assets/icons"),
    stroke: "#FFFFFF",
    size: 24,
    icons: DEFAULT_ICONS,
  };

  const a = argv.slice(2);
  for (let i = 0; i < a.length; i++) {
    switch (a[i]) {
      case "--lucide-path":
        args.lucidePath = resolve(a[++i]);
        break;
      case "--out":
        args.out = resolve(a[++i]);
        break;
      case "--stroke":
        args.stroke = a[++i];
        break;
      case "--size":
        args.size = parseInt(a[++i], 10);
        break;
      case "--icons": {
        const names = a[++i].split(",").map((s) => s.trim());
        args.icons = {};
        for (const name of names) {
          // Look up from default map, or assume name matches filename
          if (DEFAULT_ICONS[name]) {
            args.icons[name] = DEFAULT_ICONS[name];
          } else {
            args.icons[name] = `${name}.js`;
          }
        }
        break;
      }
      default:
        console.error(`Unknown option: ${a[i]}`);
        process.exit(1);
    }
  }

  return args;
}

function main() {
  const args = parseArgs(process.argv);

  if (!existsSync(args.lucidePath)) {
    console.error(`Lucide icons not found at: ${args.lucidePath}`);
    console.error(
      "Install lucide-react in ../softoboros/frontend or pass --lucide-path"
    );
    process.exit(1);
  }

  mkdirSync(args.out, { recursive: true });

  let ok = true;
  for (const [name, filename] of Object.entries(args.icons)) {
    const srcPath = join(args.lucidePath, filename);
    if (!existsSync(srcPath)) {
      console.error(`Icon not found: ${srcPath}`);
      ok = false;
      continue;
    }

    const jsSource = readFileSync(srcPath, "utf-8");
    const elements = parseIconNode(jsSource);
    const svg = buildSvg(elements, args.stroke, args.size);
    const outPath = join(args.out, `${name}.svg`);
    writeFileSync(outPath, svg, "utf-8");
    console.log(`${name}: ${elements.length} element(s) -> ${outPath}`);
  }

  if (ok) {
    console.log(`\nExtracted ${Object.keys(args.icons).length} icons to ${args.out}`);
  }
  process.exit(ok ? 0 : 1);
}

main();
