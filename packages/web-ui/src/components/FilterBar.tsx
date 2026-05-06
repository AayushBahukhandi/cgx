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

        {/* Stats */}
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
