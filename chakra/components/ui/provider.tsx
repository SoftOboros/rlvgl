// components/ui/provider.tsx - Shared Chakra and theme provider wrapper.

"use client";

import type { ReactNode } from "react";

import { ChakraProvider, defaultSystem } from "@chakra-ui/react";
import { ThemeProvider } from "next-themes";

interface ProviderProps {
  children: ReactNode;
}

export function Provider(props: ProviderProps) {
  const { children } = props;

  return (
    <ThemeProvider attribute="class" defaultTheme="dark" disableTransitionOnChange>
      <ChakraProvider value={defaultSystem}>{children}</ChakraProvider>
    </ThemeProvider>
  );
}
