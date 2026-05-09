import SearchBar from "./SearchBar";
import { NODE_COLORS, EDGE_COLORS } from "../types/graph";
import type { Community } from "../types/graph";

interface Props {
  searchQuery: string;
  onSearchChange: (q: string) => void;
  visibleKinds: Set<string>;
  onToggleKind: (kind: string) => void;
  visibleEdges: Set<string>;
  onToggleEdge: (kind: string) => void;
  communities: Community[];
  communityFilter: number | null;
  onCommunityChange: (id: number | null) => void;
  nodeCount: number;
  edgeCount: number;
  showDeadCode?: boolean;
  onToggleDeadCode?: () => void;
  deadCodeCount?: number;
}

const NODE_KINDS = ["Function", "Class", "File", "Module", "Variable", "Type", "Author"];
const EDGE_KINDS = ["CALLS", "IMPORTS", "CO_CHANGES", "OWNS"];

export default function FilterBar({
  searchQuery,
  onSearchChange,
  visibleKinds,
  onToggleKind,
  visibleEdges,
  onToggleEdge,
  communities,
  communityFilter,
  onCommunityChange,
  nodeCount,
  edgeCount,
  showDeadCode = false,
  onToggleDeadCode,
  deadCodeCount = 0,
}: Props) {
  return (
    <div
      className="flex flex-col flex-shrink-0 border-b"
      style={{ background: "#111118", borderColor: "#1e1e2e" }}
    >
      {/* Top row: search + stats */}
      <div className="flex items-center gap-3 px-3 py-2">
        {/* Search */}
        <SearchBar
          query={searchQuery}
          onChange={onSearchChange}
          placeholder="Search nodes..."
        />

        {/* Stats + GitHub link */}
        <div className="flex items-center gap-3 ml-auto">
          <span
            style={{ color: "#555570", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace" }}
          >
            {nodeCount} nodes
          </span>
          <span
            style={{ color: "#555570", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace" }}
          >
            {edgeCount} edges
          </span>
          {communityFilter !== null && (
            <span
              className="px-2 py-0.5 text-xs font-bold"
              style={{ background: "#8b5cf622", color: "#8b5cf6", fontFamily: "JetBrains Mono, monospace" }}
            >
              #{communityFilter}
            </span>
          )}
          <a
            href="https://github.com/AayushBahukhandi/cgx"
            target="_blank"
            rel="noopener noreferrer"
            title="cgx on GitHub"
            className="flex items-center opacity-50 hover:opacity-100 transition-opacity"
            style={{ color: "#8888aa" }}
          >
            <svg height="14" width="14" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
              <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/>
            </svg>
          </a>
        </div>
      </div>

      {/* Bottom row: filters */}
      <div className="flex items-center gap-2 px-3 pb-2 flex-wrap">
        {/* Node kind toggles */}
        {NODE_KINDS.map((kind) => {
          const active = visibleKinds.has(kind);
          const color = NODE_COLORS[kind] || "#888888";
          return (
            <button
              key={kind}
              onClick={() => onToggleKind(kind)}
              className="px-2 py-0.5 text-xs transition-opacity"
              style={{
                opacity: active ? 1 : 0.3,
                background: active ? `${color}22` : "transparent",
                color: active ? color : "#444466",
                border: `1px solid ${active ? color + "44" : "#1e1e2e"}`,
                fontFamily: "JetBrains Mono, monospace",
              }}
            >
              {kind}
            </button>
          );
        })}

        <span className="text-xs" style={{ color: "#333350" }}>
          |
        </span>

        {/* Edge kind toggles */}
        {EDGE_KINDS.map((kind) => {
          const active = visibleEdges.has(kind);
          const color = EDGE_COLORS[kind] || "rgba(255,255,255,0.2)";
          return (
            <button
              key={kind}
              onClick={() => onToggleEdge(kind)}
              className="px-2 py-0.5 text-xs transition-opacity"
              style={{
                opacity: active ? 1 : 0.3,
                background: active ? `${color}` : "transparent",
                color: active ? "#ddd" : "#444466",
                border: `1px solid ${active ? `${color}` : "#1e1e2e"}`,
                fontFamily: "JetBrains Mono, monospace",
              }}
            >
              {kind.replace("_", " ")}
            </button>
          );
        })}

        <span className="text-xs" style={{ color: "#333350" }}>
          |
        </span>

        {/* Dead Code toggle */}
        {onToggleDeadCode && (
          <button
            onClick={onToggleDeadCode}
            className="px-2 py-0.5 text-xs transition-opacity"
            style={{
              opacity: showDeadCode ? 1 : 0.4,
              background: showDeadCode ? "#ef444422" : "transparent",
              color: showDeadCode ? "#ef4444" : "#444466",
              border: `1px solid ${showDeadCode ? "#ef444444" : "#1e1e2e"}`,
              fontFamily: "JetBrains Mono, monospace",
            }}
            title={`Dead code overlay (${deadCodeCount} candidates)`}
          >
            dead-code{deadCodeCount > 0 ? ` (${deadCodeCount})` : ""}
          </button>
        )}

        {/* Community filter */}
        {communities.length > 0 && (
          <>
            <span className="text-xs" style={{ color: "#333350" }}>
              |
            </span>
            <select
              value={communityFilter ?? ""}
              onChange={(e) => {
                const v = e.target.value;
                onCommunityChange(v ? parseInt(v) : null);
              }}
              className="px-2 py-0.5 text-xs"
              style={{
                background: "#0a0a0f",
                color: "#8b5cf6",
                border: "1px solid #1e1e2e",
                fontFamily: "JetBrains Mono, monospace",
              }}
            >
              <option value="">All Communities</option>
              {communities.map((c) => (
                <option key={c.id} value={c.id}>
                  #{c.id} {c.label} ({c.node_count})
                </option>
              ))}
            </select>
          </>
        )}
      </div>
    </div>
  );
}
