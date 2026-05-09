export interface GraphData {
  meta: GraphMeta;
  nodes: GraphNode[];
  edges: GraphEdge[];
  communities: Community[];
}

export interface GraphMeta {
  repo_id: string;
  repo_name?: string;
  node_count: number;
  edge_count: number;
  language_breakdown: Record<string, number>;
  community_count: number;
}

export interface GraphNode {
  id: string;
  kind: NodeKind;
  name: string;
  path: string;
  line_start: number;
  line_end: number;
  language: string;
  churn: number;
  coupling: number;
  community: number;
  in_degree: number;
  out_degree: number;
  exported?: boolean;
  is_dead_candidate?: boolean;
  dead_reason?: string | null;
}

export type NodeKind =
  | "File"
  | "Function"
  | "Class"
  | "Module"
  | "Variable"
  | "Type"
  | "Author";

export interface GraphEdge {
  id: string;
  src: string;
  dst: string;
  kind: EdgeKind;
  weight: number;
  confidence: number;
}

export type EdgeKind =
  | "CALLS"
  | "IMPORTS"
  | "INHERITS"
  | "EXPORTS"
  | "CO_CHANGES"
  | "OWNS"
  | "DEPENDS_ON";

export interface Community {
  id: number;
  label: string;
  node_count: number;
  top_nodes: string[];
}

export const NODE_COLORS: Record<string, string> = {
  Function: "#00ff88",
  Class: "#3b82f6",
  File: "#f59e0b",
  Module: "#8b5cf6",
  Variable: "#34d399",
  Type: "#a855f7",
  Author: "#ec4899",
};

export const EDGE_COLORS: Record<string, string> = {
  CALLS: "rgba(255,255,255,0.13)",
  IMPORTS: "rgba(59,130,246,0.27)",
  CO_CHANGES: "rgba(239,68,68,0.40)",
  OWNS: "rgba(236,72,153,0.30)",
  INHERITS: "rgba(168,85,247,0.25)",
  EXPORTS: "rgba(52,211,153,0.20)",
  DEPENDS_ON: "rgba(245,158,11,0.25)",
};
