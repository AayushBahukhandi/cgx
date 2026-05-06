import { useMemo } from "react";
import type { GraphNode } from "../types/graph";

interface Props {
  nodes: GraphNode[];
  onSelectNode?: (id: string) => void;
}

export default function HotspotsPanel({ nodes, onSelectNode }: Props) {
  const hotspots = useMemo(() => {
    return nodes
      .filter((n) => n.kind === "File" && n.churn > 0)
      .sort((a, b) => {
        const sa = a.churn * a.coupling + a.in_degree * 0.01;
        const sb = b.churn * b.coupling + b.in_degree * 0.01;
        return sb - sa;
      })
      .slice(0, 10);
  }, [nodes]);

  if (hotspots.length === 0) {
    return (
      <div
        className="p-4 text-xs"
        style={{ color: "#555570", fontFamily: "JetBrains Mono, monospace" }}
      >
        No hotspots found. Run cgx analyze on a git repo.
      </div>
    );
  }

  return (
    <div className="p-3">
      <h3
        className="text-xs font-bold mb-2"
        style={{ color: "#ef4444", fontFamily: "JetBrains Mono, monospace" }}
      >
        HOTSPOTS
      </h3>
      <div className="space-y-1">
        {hotspots.map((n, i) => (
          <button
            key={n.id}
            onClick={() => onSelectNode?.(n.id)}
            className="w-full flex items-center gap-2 text-xs px-2 py-1 text-left hover:opacity-80 transition-opacity"
            style={{
              fontFamily: "JetBrains Mono, monospace",
              borderBottom: "1px solid #1e1e2e",
              cursor: onSelectNode ? "pointer" : "default",
            }}
          >
            <span style={{ color: "#555570", minWidth: "1.5rem" }}>{i + 1}.</span>
            <span className="flex-1 truncate" style={{ color: "#ccccdd" }}>
              {n.path}
            </span>
            <span style={{ color: "#ef4444" }}>{n.churn.toFixed(2)}</span>
            <span style={{ color: "#555570" }}>({n.in_degree})</span>
          </button>
        ))}
      </div>
    </div>
  );
}
