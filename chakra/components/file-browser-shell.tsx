// components/file-browser-shell.tsx - 747I-inspired Chakra shell for a portable file browser.

"use client";

import { startTransition, useDeferredValue, useState } from "react";

import {
  Badge,
  Box,
  Breadcrumb,
  Button,
  Checkbox,
  Dialog,
  Flex,
  Grid,
  Heading,
  HStack,
  IconButton,
  Input,
  Portal,
  ScrollArea,
  Separator,
  Table,
  Tabs,
  Text,
  TreeView,
  VStack,
  createTreeCollection,
} from "@chakra-ui/react";
import {
  LuArrowLeft,
  LuArrowRight,
  LuArrowUp,
  LuFile,
  LuFileAudio,
  LuFileCode2,
  LuFiles,
  LuFolder,
  LuHardDrive,
  LuImage,
  LuInfo,
  LuSearch,
  LuSettings2,
  LuTriangleAlert,
  LuX,
} from "react-icons/lu";

import {
  type BrowserNode,
  type BrowserTreeNode,
  type EmbeddedStatus,
  type SortKey,
  MOCK_FILESYSTEM,
  INITIAL_DIRECTORY_PATH,
  INITIAL_SELECTED_PATH,
  buildTreeNodes,
  findNodeByPath,
  formatBytes,
  getPathChain,
  listDirectoryEntries,
  mergeUniquePaths,
} from "@/components/file-browser/mock-fs";

type WingMode = "settings" | "info" | null;
type SortDirection = "asc" | "desc";

const treeCollection = createTreeCollection<BrowserTreeNode>({
  nodeToValue: (node) => node.path,
  nodeToString: (node) => node.name,
  rootNode: {
    id: "root",
    name: "Sources",
    path: "/",
    type: "root",
    children: buildTreeNodes(MOCK_FILESYSTEM),
  },
});

const RECENT_MESSAGES = [
  "Shell booted with desktop-grade file-open chrome.",
  "Mock filesystem staged with embedded-ready and convertible assets.",
  "Current demo browser limitations captured in COMPONENT_PLAN.md.",
];

function statusPalette(status: EmbeddedStatus) {
  switch (status) {
    case "ready":
      return "green";
    case "convert":
      return "yellow";
    case "unsupported":
      return "red";
    default:
      return "gray";
  }
}

function statusLabel(status: EmbeddedStatus) {
  switch (status) {
    case "ready":
      return "embedded ready";
    case "convert":
      return "needs conversion";
    case "unsupported":
      return "unsupported";
    default:
      return status;
  }
}

function nodeIcon(node: BrowserNode) {
  if (node.kind === "volume") {
    return <LuHardDrive />;
  }

  if (node.kind === "directory") {
    return <LuFolder />;
  }

  if (node.fileType.includes("audio")) {
    return <LuFileAudio />;
  }

  if (node.fileType.includes("image")) {
    return <LuImage />;
  }

  if (node.fileType.includes("JSON") || node.fileType.includes("Theme")) {
    return <LuFileCode2 />;
  }

  return <LuFile />;
}

function sortEntries(
  entries: BrowserNode[],
  sortKey: SortKey,
  sortDirection: SortDirection,
): BrowserNode[] {
  const direction = sortDirection === "asc" ? 1 : -1;

  return [...entries].sort((left, right) => {
    if (left.kind !== right.kind) {
      const leftRank = left.kind === "file" ? 1 : 0;
      const rightRank = right.kind === "file" ? 1 : 0;
      return leftRank - rightRank;
    }

    switch (sortKey) {
      case "kind":
        return left.fileType.localeCompare(right.fileType) * direction;
      case "size":
        return ((left.sizeBytes ?? -1) - (right.sizeBytes ?? -1)) * direction;
      case "modified":
        return left.modifiedAt.localeCompare(right.modifiedAt) * direction;
      case "name":
      default:
        return left.name.localeCompare(right.name) * direction;
    }
  });
}

function WingPanel(props: { mode: Exclude<WingMode, null> }) {
  const { mode } = props;

  const entries =
    mode === "settings"
      ? [
          {
            title: "Selection Policy",
            body: "Keep the browser model portable: path, selection, sort, open, cancel, and conversion state all stay backend-agnostic.",
          },
          {
            title: "Embedded Filter",
            body: "Default views should surface embedded-ready assets first and make conversion-required files explicit instead of hiding them.",
          },
          {
            title: "Compact Layout",
            body: "The desktop three-pane shell should reduce to a one-pane flow on smaller embedded displays without changing the command model.",
          },
        ]
      : [
          {
            title: "High-Value Gap",
            body: "The current demo browser is still a single-panel navigator. The replacement needs tree, breadcrumbs, details, and a stronger open flow.",
          },
          {
            title: "Current Focus",
            body: "TreeView, Table, ScrollArea, Input, Breadcrumb, and Tabs are the first reusable widgets to extract for the file browser.",
          },
          {
            title: "Portability Rule",
            body: "Do not build around browser-native OS file dialogs. The browser shell is a reference surface over a portable state machine.",
          },
        ];

  return (
    <Box
      position="fixed"
      left="4"
      top={{ base: "4", lg: "24" }}
      zIndex="dropdown"
      width={{ base: "calc(100vw - 2rem)", lg: "20rem" }}
      borderWidth="1px"
      borderColor="whiteAlpha.200"
      bg="rgba(8, 19, 29, 0.9)"
      backdropFilter="blur(18px)"
      borderRadius="2xl"
      boxShadow="0 24px 60px rgba(0, 0, 0, 0.4)"
    >
      <VStack align="stretch" gap="4" p="5">
        <HStack justify="space-between">
          <Heading size="sm" letterSpacing="0.08em" textTransform="uppercase">
            {mode === "settings" ? "Settings Wing" : "Info Wing"}
          </Heading>
          <Badge colorPalette={mode === "settings" ? "teal" : "yellow"} variant="solid">
            747I shell
          </Badge>
        </HStack>
        {entries.map((entry) => (
          <Box
            key={entry.title}
            borderWidth="1px"
            borderColor="whiteAlpha.200"
            borderRadius="xl"
            bg="blackAlpha.300"
            p="4"
          >
            <Text fontWeight="semibold" mb="2">
              {entry.title}
            </Text>
            <Text color="whiteAlpha.760" fontSize="sm" lineHeight="1.6">
              {entry.body}
            </Text>
          </Box>
        ))}
      </VStack>
    </Box>
  );
}

export function FileBrowserShell() {
  const [wingMode, setWingMode] = useState<WingMode>("info");
  const [browserOpen, setBrowserOpen] = useState(true);
  const [directoryPath, setDirectoryPath] = useState(INITIAL_DIRECTORY_PATH);
  const [selectedPath, setSelectedPath] = useState(INITIAL_SELECTED_PATH);
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [sortDirection, setSortDirection] = useState<SortDirection>("asc");
  const [embeddedOnly, setEmbeddedOnly] = useState(false);
  const [expandedValue, setExpandedValue] = useState(
    getPathChain(MOCK_FILESYSTEM, INITIAL_DIRECTORY_PATH).map((node) => node.path),
  );
  const [backStack, setBackStack] = useState<string[]>([]);
  const [forwardStack, setForwardStack] = useState<string[]>([]);
  const [searchValue, setSearchValue] = useState("");
  const [recentEvents, setRecentEvents] = useState(RECENT_MESSAGES);

  const deferredSearchValue = useDeferredValue(searchValue.trim().toLowerCase());
  const directoryChain = getPathChain(MOCK_FILESYSTEM, directoryPath);
  const selectedNode =
    findNodeByPath(MOCK_FILESYSTEM, selectedPath) ??
    findNodeByPath(MOCK_FILESYSTEM, directoryPath);

  const visibleEntries = sortEntries(
    listDirectoryEntries(MOCK_FILESYSTEM, directoryPath).filter((entry) => {
      const matchesSearch =
        deferredSearchValue.length === 0 ||
        entry.name.toLowerCase().includes(deferredSearchValue) ||
        entry.fileType.toLowerCase().includes(deferredSearchValue) ||
        entry.note.toLowerCase().includes(deferredSearchValue);

      const matchesEmbeddedRule =
        !embeddedOnly || entry.kind !== "file" || entry.embeddedStatus !== "unsupported";

      return matchesSearch && matchesEmbeddedRule;
    }),
    sortKey,
    sortDirection,
  );

  const fileNameValue = selectedNode?.kind === "file" ? selectedNode.name : "";

  function logEvent(message: string) {
    const timestamp = new Date().toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
    });

    setRecentEvents((previous) => [`${message} • ${timestamp}`, ...previous].slice(0, 6));
  }

  function syncDirectory(nextPath: string, reason: string, pushHistory = true) {
    if (nextPath === directoryPath) {
      return;
    }

    if (pushHistory) {
      setBackStack((previous) => [...previous, directoryPath]);
      setForwardStack([]);
    }

    const nextChain = getPathChain(MOCK_FILESYSTEM, nextPath).map((node) => node.path);

    startTransition(() => {
      setDirectoryPath(nextPath);
      setSelectedPath(nextPath);
      setExpandedValue((previous) => mergeUniquePaths(previous, nextChain));
    });

    logEvent(reason);
  }

  function selectEntry(entry: BrowserNode) {
    setSelectedPath(entry.path);
    logEvent(`Selected ${entry.name}`);
  }

  function activateEntry(entry: BrowserNode) {
    if (entry.kind === "file") {
      setSelectedPath(entry.path);
      setBrowserOpen(false);
      logEvent(`Queued ${entry.name} for embedded ingest`);
      return;
    }

    syncDirectory(entry.path, `Entered ${entry.name}`);
  }

  function goBack() {
    const previousPath = backStack.at(-1);
    if (!previousPath) {
      return;
    }

    setBackStack((previous) => previous.slice(0, -1));
    setForwardStack((previous) => [directoryPath, ...previous]);
    startTransition(() => {
      setDirectoryPath(previousPath);
      setSelectedPath(previousPath);
      setExpandedValue((previous) =>
        mergeUniquePaths(
          previous,
          getPathChain(MOCK_FILESYSTEM, previousPath).map((node) => node.path),
        ),
      );
    });
    logEvent(`Back to ${previousPath}`);
  }

  function goForward() {
    const nextPath = forwardStack[0];
    if (!nextPath) {
      return;
    }

    setForwardStack((previous) => previous.slice(1));
    setBackStack((previous) => [...previous, directoryPath]);
    startTransition(() => {
      setDirectoryPath(nextPath);
      setSelectedPath(nextPath);
      setExpandedValue((previous) =>
        mergeUniquePaths(
          previous,
          getPathChain(MOCK_FILESYSTEM, nextPath).map((node) => node.path),
        ),
      );
    });
    logEvent(`Forward to ${nextPath}`);
  }

  function goUp() {
    if (directoryChain.length <= 1) {
      return;
    }

    const parent = directoryChain[directoryChain.length - 2];
    syncDirectory(parent.path, `Up to ${parent.name}`);
  }

  function toggleSort(nextSortKey: SortKey) {
    if (sortKey === nextSortKey) {
      setSortDirection((previous) => (previous === "asc" ? "desc" : "asc"));
      return;
    }

    setSortKey(nextSortKey);
    setSortDirection("asc");
  }

  return (
    <Box
      minH="100dvh"
      position="relative"
      overflow="clip"
      bg="transparent"
      color="whiteAlpha.950"
      px={{ base: "4", md: "6" }}
      py={{ base: "5", md: "6" }}
    >
      <Box
        position="absolute"
        inset={{ base: "-20% 0 0 0", md: "-18% 10% auto 25%" }}
        height={{ base: "18rem", md: "28rem" }}
        borderRadius="full"
        bg="radial-gradient(circle, rgba(244, 184, 66, 0.22) 0%, transparent 65%)"
        filter="blur(40px)"
        pointerEvents="none"
      />

      <VStack align="stretch" gap="6" position="relative" zIndex="base">
        <Flex
          direction={{ base: "column", md: "row" }}
          align={{ base: "flex-start", md: "flex-end" }}
          justify="space-between"
          gap="5"
        >
          <VStack align="flex-start" gap="3" maxW="4xl">
            <Badge colorPalette="yellow" variant="subtle">
              Next.js frame for the 747I demo
            </Badge>
            <Heading size={{ base: "2xl", md: "4xl" }} lineHeight="1.02" letterSpacing="-0.04em">
              Build the richer file browser here, then reduce it to embedded-safe
              widgets later.
            </Heading>
            <Text color="whiteAlpha.760" maxW="3xl" fontSize={{ base: "sm", md: "md" }}>
              The shell mirrors the current board demo while upgrading the file
              experience to include tree navigation, breadcrumbs, sortable
              listings, metadata, preview, and explicit open/cancel semantics.
            </Text>
          </VStack>

          <HStack
            gap="3"
            borderWidth="1px"
            borderColor="whiteAlpha.200"
            borderRadius="full"
            px="4"
            py="2"
            bg="rgba(5, 12, 19, 0.72)"
            backdropFilter="blur(12px)"
          >
            <Badge colorPalette="green" variant="solid">
              shell ready
            </Badge>
            <Badge colorPalette="yellow" variant="subtle">
              portable model
            </Badge>
            <Badge colorPalette="blue" variant="subtle">
              embedded target
            </Badge>
          </HStack>
        </Flex>

        <Grid
          templateColumns={{ base: "1fr", xl: "minmax(0, 1fr) 21rem" }}
          gap="5"
          alignItems="stretch"
        >
          <Box
            borderWidth="1px"
            borderColor="whiteAlpha.200"
            borderRadius="3xl"
            bg="rgba(6, 18, 26, 0.72)"
            backdropFilter="blur(14px)"
            minH={{ base: "20rem", lg: "26rem" }}
            p={{ base: "5", md: "6" }}
            position="relative"
          >
            <VStack align="stretch" gap="5" h="full">
              <HStack justify="space-between" align="flex-start">
                <VStack align="flex-start" gap="1">
                  <Text
                    fontSize="xs"
                    color="yellow.300"
                    textTransform="uppercase"
                    letterSpacing="0.12em"
                  >
                    Reference shell
                  </Text>
                  <Heading size="lg">Desktop-grade open dialog</Heading>
                </VStack>
                <Badge colorPalette="yellow" variant="outline">
                  browser frame
                </Badge>
              </HStack>

              <Grid
                templateColumns={{ base: "1fr", md: "repeat(3, minmax(0, 1fr))" }}
                gap="4"
              >
                {[
                  {
                    label: "Current gap",
                    value: "Single panel",
                    detail: "Current demo browser is still a narrow list navigator.",
                  },
                  {
                    label: "Next widget target",
                    value: "Tree + table",
                    detail: "Both should become portable controls, not app-only code.",
                  },
                  {
                    label: "Embedded rule",
                    value: "No browser lock-in",
                    detail: "All command and selection state must survive the trip to firmware.",
                  },
                ].map((card) => (
                  <Box
                    key={card.label}
                    borderWidth="1px"
                    borderColor="whiteAlpha.160"
                    borderRadius="2xl"
                    bg="blackAlpha.300"
                    p="4"
                  >
                    <Text fontSize="xs" color="whiteAlpha.600" textTransform="uppercase">
                      {card.label}
                    </Text>
                    <Heading size="md" mt="3">
                      {card.value}
                    </Heading>
                    <Text mt="2" color="whiteAlpha.760" fontSize="sm">
                      {card.detail}
                    </Text>
                  </Box>
                ))}
              </Grid>

              <Box
                mt="auto"
                borderWidth="1px"
                borderColor="whiteAlpha.160"
                borderRadius="2xl"
                bg="linear-gradient(180deg, rgba(11, 30, 41, 0.95), rgba(7, 18, 28, 0.92))"
                p="5"
              >
                <Text fontSize="xs" color="whiteAlpha.600" textTransform="uppercase">
                  Portable interaction stack
                </Text>
                <Text mt="3" fontSize="sm" color="whiteAlpha.820" lineHeight="1.7">
                  `TreeView`, `Table`, `ScrollArea`, `Input`, `Breadcrumb`, and
                  `Tabs` are the core reusable controls. `Dialog`, event overlay,
                  and launcher chrome stay as higher-order UI.
                </Text>
              </Box>
            </VStack>
          </Box>

          <Box
            borderWidth="1px"
            borderColor="whiteAlpha.200"
            borderRadius="3xl"
            bg="rgba(6, 18, 26, 0.72)"
            backdropFilter="blur(14px)"
            p="5"
          >
            <VStack align="stretch" gap="4">
              <HStack justify="space-between">
                <Heading size="sm">Recent event window</Heading>
                <Badge colorPalette="teal" variant="subtle">
                  event overlay
                </Badge>
              </HStack>
              {recentEvents.map((message) => (
                <Box
                  key={message}
                  borderWidth="1px"
                  borderColor="whiteAlpha.160"
                  borderRadius="xl"
                  bg="blackAlpha.240"
                  px="3"
                  py="2.5"
                >
                  <Text fontSize="sm" color="whiteAlpha.820">
                    {message}
                  </Text>
                </Box>
              ))}
            </VStack>
          </Box>
        </Grid>
      </VStack>

      {wingMode ? <WingPanel mode={wingMode} /> : null}

      <Flex
        position="fixed"
        right="4"
        bottom={{ base: "4", lg: "auto" }}
        top={{ base: "auto", lg: "28" }}
        direction={{ base: "row", lg: "column" }}
        gap="3"
        zIndex="overlay"
      >
        <IconButton
          aria-label="Toggle settings wing"
          size="lg"
          borderRadius="2xl"
          bg={wingMode === "settings" ? "yellow.400" : "rgba(8, 19, 29, 0.86)"}
          color={wingMode === "settings" ? "black" : "white"}
          borderWidth="1px"
          borderColor="whiteAlpha.200"
          backdropFilter="blur(12px)"
          onClick={() => {
            setWingMode((previous) => (previous === "settings" ? null : "settings"));
            logEvent("Toggled settings wing");
          }}
        >
          <LuSettings2 />
        </IconButton>
        <IconButton
          aria-label="Open file browser"
          size="lg"
          borderRadius="2xl"
          bg="rgba(8, 19, 29, 0.86)"
          color="white"
          borderWidth="1px"
          borderColor="whiteAlpha.200"
          backdropFilter="blur(12px)"
          onClick={() => {
            setBrowserOpen(true);
            logEvent("Opened browser dialog");
          }}
        >
          <LuFiles />
        </IconButton>
        <IconButton
          aria-label="Toggle info wing"
          size="lg"
          borderRadius="2xl"
          bg={wingMode === "info" ? "teal.400" : "rgba(8, 19, 29, 0.86)"}
          color={wingMode === "info" ? "black" : "white"}
          borderWidth="1px"
          borderColor="whiteAlpha.200"
          backdropFilter="blur(12px)"
          onClick={() => {
            setWingMode((previous) => (previous === "info" ? null : "info"));
            logEvent("Toggled info wing");
          }}
        >
          <LuInfo />
        </IconButton>
      </Flex>

      <Dialog.Root
        open={browserOpen}
        placement="center"
        onOpenChange={(details) => setBrowserOpen(details.open)}
      >
        <Portal>
          <Dialog.Backdrop bg="rgba(3, 9, 14, 0.68)" backdropFilter="blur(8px)" />
          <Dialog.Positioner p={{ base: "2", md: "6" }}>
            <Dialog.Content
              width="min(96vw, 86rem)"
              maxW="unset"
              maxH="92dvh"
              overflow="hidden"
              borderWidth="1px"
              borderColor="whiteAlpha.200"
              bg="rgba(7, 17, 26, 0.98)"
              color="whiteAlpha.940"
              boxShadow="0 30px 80px rgba(0, 0, 0, 0.45)"
              borderRadius="3xl"
            >
              <Dialog.Header
                px={{ base: "4", md: "6" }}
                py="4"
                borderBottomWidth="1px"
                borderBottomColor="whiteAlpha.160"
              >
                <HStack justify="space-between" align="center">
                  <VStack align="flex-start" gap="1">
                    <Dialog.Title asChild>
                      <Heading size={{ base: "md", md: "lg" }}>
                        Open Asset for 747I Demo
                      </Heading>
                    </Dialog.Title>
                    <Text color="whiteAlpha.700" fontSize="sm">
                      Browser-first shell over a portable selection and
                      conversion model.
                    </Text>
                  </VStack>
                  <HStack gap="2">
                    <Badge colorPalette="green" variant="subtle">
                      tree + table + details
                    </Badge>
                    <Dialog.CloseTrigger asChild>
                      <IconButton
                        aria-label="Close browser"
                        variant="ghost"
                        borderRadius="full"
                      >
                        <LuX />
                      </IconButton>
                    </Dialog.CloseTrigger>
                  </HStack>
                </HStack>
              </Dialog.Header>

              <Dialog.Body px={{ base: "4", md: "6" }} py="4" overflow="hidden">
                <VStack align="stretch" gap="4" h="full">
                  <Flex
                    direction={{ base: "column", lg: "row" }}
                    justify="space-between"
                    align={{ base: "stretch", lg: "center" }}
                    gap="3"
                  >
                    <HStack flexWrap="wrap" gap="2">
                      <IconButton
                        aria-label="Back"
                        size="sm"
                        variant="outline"
                        onClick={goBack}
                        disabled={backStack.length === 0}
                      >
                        <LuArrowLeft />
                      </IconButton>
                      <IconButton
                        aria-label="Forward"
                        size="sm"
                        variant="outline"
                        onClick={goForward}
                        disabled={forwardStack.length === 0}
                      >
                        <LuArrowRight />
                      </IconButton>
                      <IconButton
                        aria-label="Up"
                        size="sm"
                        variant="outline"
                        onClick={goUp}
                        disabled={directoryChain.length <= 1}
                      >
                        <LuArrowUp />
                      </IconButton>
                      <Separator orientation="vertical" height="6" />
                      <Badge colorPalette="yellow" variant="outline">
                        current path
                      </Badge>
                    </HStack>

                    <HStack gap="3" align="stretch">
                      <Box position="relative" minW={{ base: "100%", lg: "22rem" }}>
                        <Box
                          position="absolute"
                          left="3"
                          top="50%"
                          transform="translateY(-50%)"
                          color="whiteAlpha.500"
                          pointerEvents="none"
                        >
                          <LuSearch />
                        </Box>
                        <Input
                          value={searchValue}
                          onChange={(event) => setSearchValue(event.target.value)}
                          placeholder="Search by name, type, or note"
                          pl="9"
                          bg="blackAlpha.300"
                          borderColor="whiteAlpha.200"
                        />
                      </Box>
                      <Checkbox.Root
                        checked={embeddedOnly}
                        onCheckedChange={(details) => {
                          setEmbeddedOnly(!!details.checked);
                          logEvent(
                            `${details.checked ? "Enabled" : "Disabled"} embedded-only filter`,
                          );
                        }}
                      >
                        <Checkbox.HiddenInput />
                        <Checkbox.Control />
                        <Checkbox.Label whiteSpace="nowrap">
                          Embedded-safe only
                        </Checkbox.Label>
                      </Checkbox.Root>
                    </HStack>
                  </Flex>

                  <Box
                    borderWidth="1px"
                    borderColor="whiteAlpha.160"
                    borderRadius="2xl"
                    bg="blackAlpha.240"
                    px="4"
                    py="3"
                  >
                    <Breadcrumb.Root>
                      <Breadcrumb.List flexWrap="wrap" rowGap="2">
                        {directoryChain.map((node, index) => (
                          <Box key={node.path} display="contents">
                            <Breadcrumb.Item>
                              {index === directoryChain.length - 1 ? (
                                <Breadcrumb.CurrentLink>{node.name}</Breadcrumb.CurrentLink>
                              ) : (
                                <Breadcrumb.Link
                                  href="#"
                                  onClick={(event) => {
                                    event.preventDefault();
                                    syncDirectory(node.path, `Breadcrumb to ${node.name}`);
                                  }}
                                >
                                  {node.name}
                                </Breadcrumb.Link>
                              )}
                            </Breadcrumb.Item>
                            {index < directoryChain.length - 1 ? (
                              <Breadcrumb.Separator />
                            ) : null}
                          </Box>
                        ))}
                      </Breadcrumb.List>
                    </Breadcrumb.Root>
                  </Box>

                  <Grid
                    flex="1"
                    minH="0"
                    gap="4"
                    templateColumns={{
                      base: "1fr",
                      xl: "17rem minmax(0, 1.6fr) minmax(18rem, 24rem)",
                    }}
                    alignItems="stretch"
                  >
                    <Box
                      minH={{ base: "12rem", xl: "0" }}
                      borderWidth="1px"
                      borderColor="whiteAlpha.160"
                      borderRadius="2xl"
                      bg="blackAlpha.240"
                      overflow="hidden"
                    >
                      <ScrollArea.Root height="100%">
                        <ScrollArea.Viewport>
                          <ScrollArea.Content p="4">
                            <TreeView.Root
                              collection={treeCollection}
                              selectedValue={[directoryPath]}
                              expandedValue={expandedValue}
                              onExpandedChange={(details) =>
                                setExpandedValue(details.expandedValue)
                              }
                              onSelectionChange={(details) => {
                                const nextPath = details.selectedValue[0];
                                if (nextPath) {
                                  syncDirectory(nextPath, `Tree to ${nextPath}`);
                                }
                              }}
                              size="sm"
                            >
                              <TreeView.Label srOnly>Storage tree</TreeView.Label>
                              <TreeView.Tree>
                                <TreeView.Node<BrowserTreeNode>
                                  indentGuide={<TreeView.BranchIndentGuide />}
                                  render={({ node, nodeState }) =>
                                    nodeState.isBranch ? (
                                      <TreeView.BranchControl>
                                        {node.type === "volume" ? (
                                          <LuHardDrive />
                                        ) : (
                                          <LuFolder />
                                        )}
                                        <TreeView.BranchText>{node.name}</TreeView.BranchText>
                                      </TreeView.BranchControl>
                                    ) : (
                                      <TreeView.Item>
                                        <LuFolder />
                                        <TreeView.ItemText>{node.name}</TreeView.ItemText>
                                      </TreeView.Item>
                                    )
                                  }
                                />
                              </TreeView.Tree>
                            </TreeView.Root>
                          </ScrollArea.Content>
                        </ScrollArea.Viewport>
                        <ScrollArea.Scrollbar>
                          <ScrollArea.Thumb />
                        </ScrollArea.Scrollbar>
                        <ScrollArea.Corner />
                      </ScrollArea.Root>
                    </Box>

                    <Box
                      minH={{ base: "16rem", xl: "0" }}
                      borderWidth="1px"
                      borderColor="whiteAlpha.160"
                      borderRadius="2xl"
                      bg="blackAlpha.240"
                      overflow="hidden"
                    >
                      <VStack align="stretch" gap="0" h="full">
                        <HStack
                          px="4"
                          py="3"
                          justify="space-between"
                          borderBottomWidth="1px"
                          borderBottomColor="whiteAlpha.160"
                        >
                          <Text fontWeight="semibold">Directory listing</Text>
                          <Badge colorPalette="blue" variant="subtle">
                            {visibleEntries.length} visible
                          </Badge>
                        </HStack>

                        <ScrollArea.Root flex="1" minH="0">
                          <ScrollArea.Viewport>
                            <ScrollArea.Content p="4">
                              <Table.Root size="sm">
                                <Table.Header>
                                  <Table.Row>
                                    {[
                                      { key: "name", label: "Name" },
                                      { key: "kind", label: "Type" },
                                      { key: "size", label: "Size" },
                                      { key: "modified", label: "Modified" },
                                    ].map((header) => (
                                      <Table.ColumnHeader key={header.key}>
                                        <Button
                                          variant="ghost"
                                          size="xs"
                                          onClick={() => toggleSort(header.key as SortKey)}
                                        >
                                          {header.label}
                                        </Button>
                                      </Table.ColumnHeader>
                                    ))}
                                  </Table.Row>
                                </Table.Header>
                                <Table.Body>
                                  {visibleEntries.map((entry) => (
                                    <Table.Row
                                      key={entry.path}
                                      bg={
                                        selectedPath === entry.path
                                          ? "rgba(78, 163, 122, 0.18)"
                                          : "transparent"
                                      }
                                      _hover={{ bg: "whiteAlpha.50" }}
                                      cursor="pointer"
                                      onClick={() => selectEntry(entry)}
                                      onDoubleClick={() => activateEntry(entry)}
                                    >
                                      <Table.Cell>
                                        <HStack gap="3">
                                          <Box color="whiteAlpha.700">{nodeIcon(entry)}</Box>
                                          <VStack align="flex-start" gap="0.5">
                                            <Text>{entry.name}</Text>
                                            <Text color="whiteAlpha.500" fontSize="xs">
                                              {entry.note}
                                            </Text>
                                          </VStack>
                                        </HStack>
                                      </Table.Cell>
                                      <Table.Cell>{entry.fileType}</Table.Cell>
                                      <Table.Cell>{formatBytes(entry.sizeBytes)}</Table.Cell>
                                      <Table.Cell>{entry.modifiedAt}</Table.Cell>
                                    </Table.Row>
                                  ))}
                                </Table.Body>
                              </Table.Root>
                            </ScrollArea.Content>
                          </ScrollArea.Viewport>
                          <ScrollArea.Scrollbar>
                            <ScrollArea.Thumb />
                          </ScrollArea.Scrollbar>
                          <ScrollArea.Corner />
                        </ScrollArea.Root>
                      </VStack>
                    </Box>

                    <Box
                      minH={{ base: "16rem", xl: "0" }}
                      borderWidth="1px"
                      borderColor="whiteAlpha.160"
                      borderRadius="2xl"
                      bg="blackAlpha.240"
                      overflow="hidden"
                    >
                      <Tabs.Root defaultValue="preview" fitted h="full">
                        <Tabs.List px="2" pt="2">
                          <Tabs.Trigger value="preview">Preview</Tabs.Trigger>
                          <Tabs.Trigger value="metadata">Metadata</Tabs.Trigger>
                          <Tabs.Trigger value="export">Export</Tabs.Trigger>
                        </Tabs.List>
                        <Tabs.Content value="preview" px="4" pb="4">
                          <VStack align="stretch" gap="4">
                            <HStack justify="space-between">
                              <Heading size="sm">Selection preview</Heading>
                              <Badge
                                colorPalette={statusPalette(
                                  selectedNode?.embeddedStatus ?? "unsupported",
                                )}
                                variant="subtle"
                              >
                                {statusLabel(selectedNode?.embeddedStatus ?? "unsupported")}
                              </Badge>
                            </HStack>
                            <Box
                              borderWidth="1px"
                              borderColor="whiteAlpha.160"
                              borderRadius="xl"
                              bg="rgba(2, 9, 15, 0.8)"
                              p="4"
                            >
                              <Text color="whiteAlpha.700" fontSize="sm" mb="3">
                                {selectedNode?.note}
                              </Text>
                              <Box
                                as="pre"
                                whiteSpace="pre-wrap"
                                fontSize="xs"
                                lineHeight="1.7"
                                color="teal.100"
                              >
                                {selectedNode?.preview}
                              </Box>
                            </Box>
                          </VStack>
                        </Tabs.Content>
                        <Tabs.Content value="metadata" px="4" pb="4">
                          <VStack align="stretch" gap="4">
                            <Heading size="sm">Metadata</Heading>
                            <Grid templateColumns="repeat(2, minmax(0, 1fr))" gap="3">
                              {[
                                ["Path", selectedNode?.path ?? "—"],
                                ["Type", selectedNode?.fileType ?? "—"],
                                ["Modified", selectedNode?.modifiedAt ?? "—"],
                                ["Size", formatBytes(selectedNode?.sizeBytes)],
                              ].map(([label, value]) => (
                                <Box
                                  key={label}
                                  borderWidth="1px"
                                  borderColor="whiteAlpha.160"
                                  borderRadius="xl"
                                  p="3"
                                  bg="blackAlpha.260"
                                >
                                  <Text fontSize="xs" color="whiteAlpha.560" textTransform="uppercase">
                                    {label}
                                  </Text>
                                  <Text mt="2" fontSize="sm">
                                    {value}
                                  </Text>
                                </Box>
                              ))}
                            </Grid>
                          </VStack>
                        </Tabs.Content>
                        <Tabs.Content value="export" px="4" pb="4">
                          <VStack align="stretch" gap="4">
                            <Heading size="sm">Embedded formats</Heading>
                            <HStack flexWrap="wrap" gap="2">
                              {(selectedNode?.embeddedFormats ?? []).map((format) => (
                                <Badge key={format} colorPalette="yellow" variant="outline">
                                  {format}
                                </Badge>
                              ))}
                            </HStack>
                            <Text fontSize="sm" color="whiteAlpha.760" lineHeight="1.7">
                              Use this panel to expose conversion workflows later.
                              The portable model should know whether a file is
                              directly usable, convertible, or blocked.
                            </Text>
                            {(selectedNode?.embeddedStatus ?? "unsupported") === "unsupported" ? (
                              <HStack
                                align="flex-start"
                                gap="3"
                                borderWidth="1px"
                                borderColor="red.400/30"
                                borderRadius="xl"
                                p="3"
                                bg="red.500/10"
                              >
                                <Box mt="0.5">
                                  <LuTriangleAlert />
                                </Box>
                                <Text fontSize="sm" color="whiteAlpha.820">
                                  This source is useful as a reference asset, but
                                  it is not an embedded-ready selection target yet.
                                </Text>
                              </HStack>
                            ) : null}
                          </VStack>
                        </Tabs.Content>
                      </Tabs.Root>
                    </Box>
                  </Grid>
                </VStack>
              </Dialog.Body>

              <Dialog.Footer
                px={{ base: "4", md: "6" }}
                py="4"
                borderTopWidth="1px"
                borderTopColor="whiteAlpha.160"
              >
                <Flex
                  direction={{ base: "column", md: "row" }}
                  justify="space-between"
                  align={{ base: "stretch", md: "center" }}
                  gap="4"
                  w="full"
                >
                  <VStack align="stretch" gap="2" flex="1">
                    <Text fontSize="xs" color="whiteAlpha.560" textTransform="uppercase">
                      Selected file
                    </Text>
                    <Input
                      value={fileNameValue}
                      readOnly
                      placeholder="Choose a file to open"
                      bg="blackAlpha.300"
                      borderColor="whiteAlpha.200"
                    />
                  </VStack>
                  <HStack justify="flex-end" gap="3">
                    <Button
                      variant="outline"
                      onClick={() => {
                        setBrowserOpen(false);
                        logEvent("Dismissed browser dialog");
                      }}
                    >
                      Cancel
                    </Button>
                    <Button
                      colorPalette="yellow"
                      onClick={() => {
                        if (selectedNode) {
                          activateEntry(selectedNode);
                        }
                      }}
                      disabled={selectedNode?.kind !== "file"}
                    >
                      Open for Demo
                    </Button>
                  </HStack>
                </Flex>
              </Dialog.Footer>
            </Dialog.Content>
          </Dialog.Positioner>
        </Portal>
      </Dialog.Root>
    </Box>
  );
}
