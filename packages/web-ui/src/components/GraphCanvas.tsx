import { useEffect, useRef, useCallback } from "react";
import Sigma from "sigma";
import Graph from "graphology";
import forceAtlas2 from "graphology-layout-forceatlas2";
import type { GraphData } from "../types/graph";
import { NODE_COLORS, EDGE_COLORS } from "../types/graph";

interface Props {
  data: GraphData;
  selectedId: string | null;
  onSelectNode: (id: string | null) => void;
  visibleKinds: Set<string>;
  visibleEdges: Set<string>;
  searchQuery: string;
  communityFilter: number | null;
  showDeadCode?: boolean;
}

export default function GraphCanvas({
  data,
  selectedId,
  onSelectNode,
  visibleKinds,
  visibleEdges,
  searchQuery,
  communityFilter,
  showDeadCode = false,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const sigmaRef = useRef<Sigma | null>(null);
  const graphRef = useRef<Graph | null>(null);

  // Keep latest filter state accessible in callbacks without stale closures
  const visibleKindsRef = useRef(visibleKinds);
  const visibleEdgesRef = useRef(visibleEdges);
  const searchQueryRef = useRef(searchQuery);
  const communityFilterRef = useRef(communityFilter);
  const dataRef = useRef(data);
  const showDeadCodeRef = useRef(showDeadCode);

  visibleKindsRef.current = visibleKinds;
  visibleEdgesRef.current = visibleEdges;
  searchQueryRef.current = searchQuery;
  communityFilterRef.current = communityFilter;
  dataRef.current = data;
  showDeadCodeRef.current = showDeadCode;

  const computeVisibleNodeIds = useCallback((): Set<string> => {
    const ids = new Set<string>();
    for (const n of dataRef.current.nodes) {
      if (!visibleKindsRef.current.has(n.kind)) continue;
      if (communityFilterRef.current !== null && n.community !== communityFilterRef.current) continue;
      if (searchQueryRef.current) {
        const q = searchQueryRef.current.toLowerCase();
        if (
          !n.name.toLowerCase().includes(q) &&
          !n.path.toLowerCase().includes(q) &&
          !n.kind.toLowerCase().includes(q)
        )
          continue;
      }
      ids.add(n.id);
    }
    return ids;
  }, []);

  const applyVisibility = useCallback(() => {
    const graph = graphRef.current;
    const sigma = sigmaRef.current;
    if (!graph || !sigma) return;

    const desired = computeVisibleNodeIds();
    const deadOverlay = showDeadCodeRef.current;

    graph.forEachNode((node, attrs) => {
      try {
        graph.setNodeAttribute(node, "hidden", !desired.has(node));
        // Update color based on dead code overlay state
        const isDeadCandidate = attrs.isDeadCandidate as boolean | undefined;
        if (deadOverlay && isDeadCandidate) {
          graph.setNodeAttribute(node, "color", "#ef4444");
        } else {
          const kind = attrs.kind as string;
          graph.setNodeAttribute(node, "color", NODE_COLORS[kind] || "#888888");
        }
      } catch {}
    });

    graph.forEachEdge((edge, attrs) => {
      const src = graph.source(edge);
      const dst = graph.target(edge);
      const edgeAllowed = visibleEdgesRef.current.has(attrs.kind as string);
      const shouldShow = edgeAllowed && desired.has(src) && desired.has(dst);
      try {
        graph.setEdgeAttribute(edge, "hidden", !shouldShow);
      } catch {}
    });

    sigma.refresh();
  }, [computeVisibleNodeIds]);

  const initSigma = useCallback(() => {
    const container = containerRef.current;
    if (!container || !dataRef.current) return;
    if (sigmaRef.current) return; // already initialized

    const d = dataRef.current;
    const g = new Graph({ multi: true });
    graphRef.current = g;

    const maxChurn = Math.max(...d.nodes.map((n) => n.churn), 0.01);
    for (const node of d.nodes) {
      // Dead code overlay: highlight dead candidates in red when overlay is active
      const isDeadCandidate = node.is_dead_candidate === true;
      const color = (showDeadCode && isDeadCandidate)
        ? "#ef4444"
        : NODE_COLORS[node.kind] || "#888888";
      const size = 4 + (node.churn / maxChurn) * 14;
      g.addNode(node.id, {
        label: node.name,
        size,
        color,
        x: Math.random() * 50,
        y: Math.random() * 50,
        hidden: true,
        kind: node.kind,
        churn: node.churn,
        coupling: node.coupling,
        community: node.community,
        inDegree: node.in_degree,
        outDegree: node.out_degree,
        path: node.path,
        lineStart: node.line_start,
        lineEnd: node.line_end,
        isDeadCandidate: node.is_dead_candidate ?? false,
        deadReason: node.dead_reason ?? null,
      });
    }

    for (const edge of d.edges) {
      if (g.hasNode(edge.src) && g.hasNode(edge.dst)) {
        const color = EDGE_COLORS[edge.kind] || "rgba(255,255,255,0.1)";
        g.addEdge(edge.src, edge.dst, {
          color,
          size: 0.3 + edge.weight * 1.5,
          hidden: true,
          kind: edge.kind,
          weight: edge.weight,
        });
      }
    }

    const s = new Sigma(g, container, {
      allowInvalidContainer: true,
      renderEdgeLabels: false,
      enableEdgeEvents: false,
      labelDensity: 0.3,
      labelRenderedSizeThreshold: 8,
      defaultNodeColor: "#888888",
      defaultEdgeColor: "rgba(255,255,255,0.1)",
      labelFont: "JetBrains Mono, monospace",
      labelColor: { color: "#aaaaaa" },
      stagePadding: 40,
    });
    sigmaRef.current = s;

    s.on("downNode", ({ node }) => onSelectNode(node));
    s.on("clickStage", () => onSelectNode(null));

    // Run layout on all nodes, then apply current filters
    if (g.order >= 2) {
      g.forEachNode((node) => {
        try { g.setNodeAttribute(node, "hidden", false); } catch {}
      });

      try {
        const settings = forceAtlas2.inferSettings(g);
        forceAtlas2.assign(g, {
          iterations: 300,
          settings: { ...settings, gravity: 2, scalingRatio: 8, slowDown: 5 },
        });
      } catch {}
    }

    // Apply current visibility state
    applyVisibility();

    const camera = s.getCamera();
    camera.animatedReset({ duration: 300 });
  }, [onSelectNode, applyVisibility]);

  // Initialize or reinitialize sigma when data changes
  useEffect(() => {
    const container = containerRef.current;
    if (!container || !data) return;

    // Tear down existing instance
    if (sigmaRef.current) {
      sigmaRef.current.kill();
      sigmaRef.current = null;
      graphRef.current = null;
    }

    if (container.clientHeight > 0) {
      initSigma();
      return () => {
        if (sigmaRef.current) {
          sigmaRef.current.kill();
          sigmaRef.current = null;
          graphRef.current = null;
        }
      };
    }

    // Container has no height yet — wait for it
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        if (entry.contentRect.height > 0) {
          ro.disconnect();
          initSigma();
          break;
        }
      }
    });
    ro.observe(container);

    return () => {
      ro.disconnect();
      if (sigmaRef.current) {
        sigmaRef.current.kill();
        sigmaRef.current = null;
        graphRef.current = null;
      }
    };
  }, [data, initSigma]);

  // Update visibility when filters change
  useEffect(() => {
    applyVisibility();
  }, [visibleKinds, visibleEdges, searchQuery, communityFilter, showDeadCode, applyVisibility]);

  // Highlight selected node
  useEffect(() => {
    const graph = graphRef.current;
    const sigma = sigmaRef.current;
    if (!graph || !sigma) return;

    graph.forEachNode((node) => {
      try { graph.removeNodeAttribute(node, "highlighted"); } catch {}
    });

    if (selectedId) {
      try { graph.setNodeAttribute(selectedId, "highlighted", true); } catch {}
    }

    sigma.refresh();
  }, [selectedId]);

  // Handle resize
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver(() => { sigmaRef.current?.refresh(); });
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

  return (
    <div
      ref={containerRef}
      className="w-full h-full"
      style={{ background: "#0a0a0f" }}
    />
  );
}
