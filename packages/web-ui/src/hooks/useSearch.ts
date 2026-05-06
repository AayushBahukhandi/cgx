import { useMemo } from "react";
import type { GraphNode } from "../types/graph";

export function useSearch(nodes: GraphNode[], query: string) {
  return useMemo(() => {
    const q = query.toLowerCase().trim();
    if (!q) return nodes;
    return nodes.filter(
      (n) =>
        n.name.toLowerCase().includes(q) ||
        n.path.toLowerCase().includes(q) ||
        n.kind.toLowerCase().includes(q)
    );
  }, [nodes, query]);
}
