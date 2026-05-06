import { useEffect, useState } from "react";
import type { GraphData } from "../types/graph";

declare global {
  interface Window {
    __CGX_GRAPH__?: GraphData;
  }
}

export function useGraph() {
  const [data, setData] = useState<GraphData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        // Priority 1: baked-in graph data (publish mode)
        if (window.__CGX_GRAPH__) {
          if (!cancelled) {
            setData(window.__CGX_GRAPH__);
            setLoading(false);
          }
          return;
        }

        // Priority 2: fetch from API
        const resp = await fetch("/api/graph");
        if (!resp.ok) {
          throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
        }
        const json: GraphData | { error: string } = await resp.json();

        if ("error" in json) {
          throw new Error(json.error);
        }

        if (!cancelled) {
          setData(json as GraphData);
          setLoading(false);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "Failed to load graph data");
          setLoading(false);
        }
      }
    }

    load();
    return () => {
      cancelled = true;
    };
  }, []);

  return { data, loading, error };
}
