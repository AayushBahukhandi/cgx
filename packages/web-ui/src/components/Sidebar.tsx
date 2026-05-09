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
        {/* Resources footer */}
        <div
          className="p-3 border-t flex-shrink-0 space-y-1.5"
          style={{ borderColor: "#1e1e2e" }}
        >
          <p className="text-xs font-bold" style={{ color: "#444466", fontFamily: "Syne, sans-serif" }}>
            RESOURCES
          </p>
          <div className="flex flex-col gap-1">
            <a
              href="https://github.com/AayushBahukhandi/cgx"
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-1.5 hover:opacity-80 transition-opacity"
              style={{ color: "#555570", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace" }}
            >
              <svg height="11" width="11" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true" style={{ flexShrink: 0 }}>
                <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/>
              </svg>
              AayushBahukhandi/cgx
            </a>
            <a
              href="https://github.com/AayushBahukhandi/cgx/issues/new"
              target="_blank"
              rel="noopener noreferrer"
              className="hover:opacity-80 transition-opacity"
              style={{ color: "#555570", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace" }}
            >
              ↗ Open an issue
            </a>
            <a
              href="https://github.com/AayushBahukhandi/cgx#readme"
              target="_blank"
              rel="noopener noreferrer"
              className="hover:opacity-80 transition-opacity"
              style={{ color: "#555570", fontSize: "0.6875rem", fontFamily: "JetBrains Mono, monospace" }}
            >
              ↗ Documentation
            </a>
          </div>
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

      {/* Dead code warning banner */}
      {node.is_dead_candidate && (
        <div
          className="px-3 py-2 flex-shrink-0"
          style={{
            background: "#ef444415",
            borderTop: "1px solid #ef444433",
            borderBottom: "1px solid #ef444433",
          }}
        >
          <p
            className="text-xs font-bold"
            style={{ color: "#ef4444", fontFamily: "JetBrains Mono, monospace" }}
          >
            DEAD CODE CANDIDATE
          </p>
          {node.dead_reason && (
            <p
              className="text-xs mt-0.5"
              style={{ color: "#ef444499", fontFamily: "JetBrains Mono, monospace" }}
            >
              reason: {node.dead_reason}
            </p>
          )}
          <p
            className="text-xs mt-1"
            style={{ color: "#6b7280", fontFamily: "JetBrains Mono, monospace" }}
          >
            Nothing references this symbol. Verify before deleting.
          </p>
        </div>
      )}

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
