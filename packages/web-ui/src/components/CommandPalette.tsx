import { useState, useEffect, useRef } from "react";
import type { GraphNode } from "../types/graph";

interface Props {
  nodes: GraphNode[];
  onSelectNode: (id: string) => void;
  onClose: () => void;
}

export default function CommandPalette({ nodes, onSelectNode, onClose }: Props) {
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [onClose]);

  const results = nodes
    .filter(
      (n) =>
        query.length > 0 &&
        (n.name.toLowerCase().includes(query.toLowerCase()) ||
          n.path.toLowerCase().includes(query.toLowerCase()))
    )
    .slice(0, 12);

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-20"
      style={{ background: "rgba(0,0,0,0.6)" }}
      onClick={onClose}
    >
      <div
        className="w-full max-w-lg border overflow-hidden"
        style={{ background: "#111118", borderColor: "#1e1e2e" }}
        onClick={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search nodes..."
          className="w-full px-4 py-3 text-sm outline-none"
          style={{
            background: "#0a0a0f",
            color: "#ccccdd",
            borderBottom: "1px solid #1e1e2e",
            fontFamily: "JetBrains Mono, monospace",
          }}
        />
        <div className="max-h-64 overflow-y-auto">
          {results.length === 0 && query.length > 0 && (
            <div
              className="px-4 py-3 text-xs"
              style={{ color: "#555570", fontFamily: "JetBrains Mono, monospace" }}
            >
              No results
            </div>
          )}
          {results.map((n) => (
            <button
              key={n.id}
              className="w-full text-left px-4 py-2 text-xs hover:bg-white/5 transition-colors"
              style={{
                color: "#ccccdd",
                fontFamily: "JetBrains Mono, monospace",
                borderBottom: "1px solid #1e1e2e",
              }}
              onClick={() => {
                onSelectNode(n.id);
                onClose();
              }}
            >
              <span className="font-bold">{n.name}</span>
              <span className="ml-2" style={{ color: "#555570" }}>
                {n.kind} — {n.path}
              </span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
