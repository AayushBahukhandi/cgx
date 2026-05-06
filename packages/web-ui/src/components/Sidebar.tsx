import HotspotsPanel from "./HotspotsPanel";
import SnippetPreview from "./SnippetPreview";
import type { GraphNode } from "../types/graph";
import { NODE_COLORS } from "../types/graph";

interface Props {
  node: GraphNode | null;
  callers: GraphNode[];
  callees: GraphNode[];
  nodes: GraphNode[];
  onSelectNode: (id: string | null) => void;
}

export default function Sidebar({ node, callers, callees, nodes, onSelectNode }: Props) {
  if (!node) {
    return (
      <div className="flex flex-col h-full overflow-hidden" style={{ background: "#111118" }}>
        <div
          className="p-3 border-b flex-shrink-0"
          style={{ borderColor: "#1e1e2e" }}
        >
          <h2 className="text-sm font-bold" style={{ color: "#8888aa", fontFamily: "Syne, sans-serif" }}>
            INSPECTOR
          </h2>
        </div>
        <div className="flex-1 overflow-y-auto">
          <HotspotsPanel nodes={nodes} onSelectNode={onSelectNode} />
        </div>
      </div>
    );
  }

  const color = NODE_COLORS[node.kind] || "#888888";
  const churnPct = Math.min(node.churn * 100, 100);
  const coupPct = Math.min(node.coupling * 100, 100);

  return (
    <div className="flex flex-col h-full overflow-hidden" style={{ background: "#111118" }}>
      {/* Header */}
      <div
        className="p-3 border-b flex-shrink-0"
        style={{ borderColor: "#1e1e2e" }}
      >
        <h2 className="text-sm font-bold mb-2" style={{ color: "#8888aa", fontFamily: "Syne, sans-serif" }}>
          INSPECTOR
        </h2>
        <div className="flex items-center gap-2">
          <span
            className="px-2 py-0.5 text-xs font-bold"
            style={{
              background: color,
              color: "#000",
              fontFamily: "JetBrains Mono, monospace",
            }}
          >
            {node.kind.toUpperCase()}
          </span>
          <span
            className="text-sm font-bold truncate"
            style={{ color, fontFamily: "JetBrains Mono, monospace" }}
          >
            {node.name}
          </span>
        </div>
      </div>

      {/* Details */}
      <div
        className="p-3 border-b flex-shrink-0 space-y-2"
        style={{ borderColor: "#1e1e2e" }}
      >
        {node.path && (
          <div className="flex gap-2">
            <span style={{ color: "#555570", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace" }}>
              File:
            </span>
            <span style={{ color: "#8888aa", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace" }} className="truncate">
              {node.path}
            </span>
          </div>
        )}
        {node.line_start > 0 && (
          <div className="flex gap-2">
            <span style={{ color: "#555570", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace" }}>
              Lines:
            </span>
            <span style={{ color: "#8888aa", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace" }}>
              {node.line_start}-{node.line_end}
            </span>
          </div>
        )}
        {/* Churn bar */}
        <div className="flex items-center gap-2">
          <span style={{ color: "#555570", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace", width: "44px" }}>
            Churn:
          </span>
          <div className="flex-1 h-2.5" style={{ background: "#1a1a25" }}>
            <div
              className="h-full transition-all"
              style={{ width: `${churnPct}%`, background: "#ef4444" }}
            />
          </div>
          <span style={{ color: "#8888aa", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace", width: "36px", textAlign: "right" }}>
            {node.churn.toFixed(2)}
          </span>
        </div>
        {/* Coupling bar */}
        <div className="flex items-center gap-2">
          <span style={{ color: "#555570", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace", width: "44px" }}>
            Coup:
          </span>
          <div className="flex-1 h-2.5" style={{ background: "#1a1a25" }}>
            <div
              className="h-full transition-all"
              style={{ width: `${coupPct}%`, background: "#3b82f6" }}
            />
          </div>
          <span style={{ color: "#8888aa", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace", width: "36px", textAlign: "right" }}>
            {node.coupling.toFixed(2)}
          </span>
        </div>
        {node.community > 0 && (
          <div className="flex gap-2">
            <span style={{ color: "#555570", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace" }}>
              Comm:
            </span>
            <span style={{ color: "#8b5cf6", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace" }} className="font-bold">
              #{node.community}
            </span>
          </div>
        )}
        <div className="flex gap-2">
          <span style={{ color: "#555570", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace" }}>
            in:{node.in_degree}
          </span>
          <span style={{ color: "#555570", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace" }}>
            out:{node.out_degree}
          </span>
        </div>
      </div>

      {/* Source snippet preview */}
      {node.path && (
        <SnippetPreview
          path={node.path}
          lineStart={node.line_start}
          lineEnd={node.line_end}
        />
      )}

      {/* Callers */}
      <div className="flex-1 overflow-y-auto min-h-0">
        <NodeList
          title="CALLERS"
          nodes={callers}
          onSelect={onSelectNode}
        />
        <NodeList
          title="CALLEES"
          nodes={callees}
          onSelect={onSelectNode}
        />
      </div>
    </div>
  );
}

function NodeList({
  title,
  nodes,
  onSelect,
}: {
  title: string;
  nodes: GraphNode[];
  onSelect: (id: string | null) => void;
}) {
  return (
    <div
      className="border-b"
      style={{ borderColor: "#1e1e2e" }}
    >
      <div
        className="px-3 py-1.5 text-xs font-bold"
        style={{ color: "#444466", fontFamily: "Syne, sans-serif" }}
      >
        {title} {nodes.length > 0 && `(${nodes.length})`}
      </div>
      {nodes.length === 0 ? (
        <p className="px-3 pb-2 text-xs" style={{ color: "#333350", fontFamily: "JetBrains Mono, monospace" }}>
          (none)
        </p>
      ) : (
        <div className="max-h-44 overflow-y-auto">
          {nodes.slice(0, 12).map((n) => {
            const c = NODE_COLORS[n.kind] || "#888888";
            return (
              <button
                key={n.id}
                onClick={() => onSelect(n.id)}
                className="w-full text-left px-3 py-1 hover:opacity-80 flex items-center gap-2 transition-colors"
                style={{ fontFamily: "JetBrains Mono, monospace", fontSize: "0.6875rem" }}
              >
                <span
                  className="w-1.5 h-1.5 rounded-full flex-shrink-0"
                  style={{ background: c }}
                />
                <span className="truncate" style={{ color: c }}>
                  {n.name}
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
