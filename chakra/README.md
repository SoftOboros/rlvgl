<!--
README.md - Overview and workflow notes for the Chakra file browser lab.
-->

# Chakra File Browser Lab

This directory is a standalone Next.js 16 + Chakra UI 3 app that acts as a
reference shell for a richer `747i-demo` file browser. The goal is not a
browser-only toy dialog. The goal is a portable interaction model that can
later collapse into an embedded-safe implementation.

## What This App Proves

- a desktop-grade file-open shell can be expressed as a composition of reusable
  controls instead of one monolithic screen
- the high-value behaviors for the current `rlvgl` demo are tree navigation,
  breadcrumbs, sortable listings, metadata, preview, and explicit open/cancel
  semantics
- the browser shell can stay separate from the file-browser state model so the
  later embedded version is not forced to copy web-only assumptions

## Run It

```bash
cd chakra
npm install
npm run dev
```

The app runs on [http://localhost:3000](http://localhost:3000).

## Validate It

```bash
cd chakra
npm run lint
npm run build
```

The scripts use `--webpack` for `dev` and `build`, matching Chakra UI's current
Next.js guidance for avoiding Emotion hydration issues.

## Design Notes

- The shell mirrors the `stm32h747i-disco` demo structure: right icon strip,
  left-side wings, central modal browser, and a small event log overlay.
- The mock filesystem is intentionally embedded-oriented. It mixes ready,
  convertible, and unsupported assets so the UI can expose the same decisions
  a future on-device browser will need to make.
- The component split and rollout plan live in [COMPONENT_PLAN.md](./COMPONENT_PLAN.md).

## Sources

- [Chakra UI Next.js App guide](https://chakra-ui.com/docs/get-started/frameworks/next-app)
- [Chakra UI component catalog](https://chakra-ui.com/llms-components.txt)
- [Next.js App Router docs](https://nextjs.org/docs/app/getting-started)
