// app/layout.tsx - Root layout for the Chakra-backed 747I demo shell.

import type { Metadata } from "next";
import { IBM_Plex_Mono, Space_Grotesk } from "next/font/google";

import { Provider } from "@/components/ui/provider";
import "./globals.css";

const displayFont = Space_Grotesk({
  variable: "--font-display",
  subsets: ["latin"],
});

const monoFont = IBM_Plex_Mono({
  variable: "--font-mono",
  subsets: ["latin"],
  weight: ["400", "500"],
});

export const metadata: Metadata = {
  title: "rlvgl Chakra File Browser Lab",
  description:
    "A Next.js + Chakra UI reference shell for a desktop-grade file browser that can later reduce to embedded-safe interactions.",
};

export default function RootLayout(props: Readonly<{ children: React.ReactNode }>) {
  const { children } = props;

  return (
    <html
      lang="en"
      suppressHydrationWarning
      className={`${displayFont.variable} ${monoFont.variable}`}
    >
      <body>
        <Provider>{children}</Provider>
      </body>
    </html>
  );
}
