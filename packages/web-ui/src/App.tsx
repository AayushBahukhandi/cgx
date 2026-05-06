import { useState, useMemo, useCallback, useEffect } from "react";
import { useGraph } from "./hooks/useGraph";
import GraphCanvas from "./components/GraphCanvas";
import Sidebar from "./components/Sidebar";
import FilterBar from "./components/FilterBar";
import CommandPalette from "./components/CommandPalette";
import ChatPanel from "./components/ChatPanel";
import type { GraphNode } from "./types/graph";

export default function App() {
  const { data, loading, error } = useGraph();

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [visibleKinds, setVisibleKinds] = useState<Set<string>>(
    new Set(["Function", "Class", "File", "Module", "Variable", "Type", "Author"])
  );
  const [visibleEdges, setVisibleEdges] = useState<Set<string>>(
    new Set(["CALLS", "IMPORTS", "CO_CHANGES"])
  );
  const [communityFilter, setCommunityFilter] = useState<number | null>(null);
  const [showPalette, setShowPalette] = useState(false);

  const handleToggleKind = useCallback((kind: string) => {
    setVisibleKinds((prev) => {
      const next = new Set(prev);
      if (next.has(kind)) next.delete(kind);
      else next.add(kind);
      return next;
    });
  }, []);

  const handleToggleEdge = useCallback((kind: string) => {
    setVisibleEdges((prev) => {
      const next = new Set(prev);
      if (next.has(kind)) next.delete(kind);
      else next.add(kind);
      return next;
    });
  }, []);

  // Cmd+K toggles command palette
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        setShowPalette((prev) => !prev);
      }
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, []);

  // Compute callers and callees for selected node
  const { callers, callees, nodeCount, edgeCount } = useMemo(() => {
    if (!data) return { callers: [], callees: [], nodeCount: 0, edgeCount: 0 };

    const nodeMap = new Map<string, GraphNode>();
    for (const n of data.nodes) nodeMap.set(n.id, n);

    let callers: GraphNode[] = [];
    let callees: GraphNode[] = [];

    if (selectedId) {
      for (const e of data.edges) {
        if (e.dst === selectedId) {
          const src = nodeMap.get(e.src);
          if (src) callers.push(src);
        }
        if (e.src === selectedId) {
          const dst = nodeMap.get(e.dst);
          if (dst) callees.push(dst);
        }
      }
      // Sort by in_degree descending (most important callers first)
      callers.sort((a, b) => b.in_degree - a.in_degree);
      callees.sort((a, b) => b.in_degree - a.in_degree);
    }

    // Count visible nodes/edges
    const filteredNodeIds = new Set(
      data.nodes
        .filter((n) => {
          if (!visibleKinds.has(n.kind)) return false;
          if (communityFilter !== null && n.community !== communityFilter) return false;
          if (searchQuery) {
            const q = searchQuery.toLowerCase();
            return n.name.toLowerCase().includes(q) || n.path.toLowerCase().includes(q);
          }
          return true;
        })
        .map((n) => n.id)
    );

    const filteredEdges = data.edges.filter(
      (e) =>
        visibleEdges.has(e.kind) &&
        filteredNodeIds.has(e.src) &&
        filteredNodeIds.has(e.dst)
    );

    return {
      callers,
      callees,
      nodeCount: filteredNodeIds.size,
      edgeCount: filteredEdges.length,
    };
  }, [data, selectedId, visibleKinds, visibleEdges, communityFilter, searchQuery]);

  const selectedNode = useMemo(() => {
    if (!data || !selectedId) return null;
    return data.nodes.find((n) => n.id === selectedId) || null;
  }, [data, selectedId]);

  // Loading state
  if (loading) {
    return (
      <div className="w-full h-screen flex items-center justify-center" style={{ background: "#0a0a0f" }}>
        <div className="text-center">
          <div className="w-8 h-8 mx-auto mb-3 rounded-full border-2 animate-spin" style={{ borderColor: "#1e1e2e", borderTopColor: "#00ff88" }} />
          <p style={{ color: "#555570", fontFamily: "JetBrains Mono, monospace", fontSize: "0.8125rem" }}>
            Loading graph...
          </p>
        </div>
      </div>
    );
  }

  // Error state
  if (error || !data) {
    return (
      <div className="w-full h-screen flex items-center justify-center" style={{ background: "#0a0a0f" }}>
        <div className="text-center max-w-md px-4">
          <p style={{ color: "#ef4444", fontFamily: "JetBrains Mono, monospace", fontSize: "0.875rem" }} className="mb-2">
            Failed to load graph
          </p>
          <p style={{ color: "#555570", fontFamily: "JetBrains Mono, monospace", fontSize: "0.75rem" }}>
            {error || "No data available. Run `cgx analyze` to index a repository, then `cgx serve` to start the server."}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="w-full h-screen flex flex-col" style={{ background: "#0a0a0f" }}>
      {/* Filter bar */}
      <FilterBar
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        visibleKinds={visibleKinds}
        onToggleKind={handleToggleKind}
        visibleEdges={visibleEdges}
        onToggleEdge={handleToggleEdge}
        communities={data.communities || []}
        communityFilter={communityFilter}
        onCommunityChange={setCommunityFilter}
        nodeCount={nodeCount}
        edgeCount={edgeCount}
      />

      {/* Main content: graph + sidebar */}
      <div className="flex-1 flex min-h-0">
        {/* Graph canvas */}
        <div className="flex-1 min-w-0" style={{ height: "100%" }}>
          <GraphCanvas
            data={data}
            selectedId={selectedId}
            onSelectNode={setSelectedId}
            visibleKinds={visibleKinds}
            visibleEdges={visibleEdges}
            searchQuery={searchQuery}
            communityFilter={communityFilter}
          />
        </div>

        {/* Sidebar inspector */}
        <div
          className="flex-shrink-0 border-l"
          style={{ borderColor: "#1e1e2e", width: "420px" }}
        >
          <Sidebar
            node={selectedNode}
            callers={callers}
            callees={callees}
            nodes={data.nodes}
            onSelectNode={setSelectedId}
          />
        </div>
      </div>

      {/* Command Palette */}
      {showPalette && (
        <CommandPalette
          nodes={data.nodes}
          onSelectNode={(id) => {
            setSelectedId(id);
            setShowPalette(false);
          }}
          onClose={() => setShowPalette(false)}
        />
      )}

      {/* Chat Panel */}
      <ChatPanel
        graphData={data}
        selectedNode={selectedNode}
        onSelectNode={setSelectedId}
      />
    </div>
  );
}
