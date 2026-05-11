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
        // Priority 1: ?data=URL param (cgx share mode — load from remote JSON)
        // Must be checked before baked-in data so cgx share links always work
        // even when the page was published with cgx publish
        const params = new URLSearchParams(window.location.search);
        const remoteUrl = params.get("data");
        if (remoteUrl) {
          const resp = await fetch(remoteUrl);
          if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
          const json: GraphData | { error: string } = await resp.json();
          if (!cancelled) {
            if ("error" in json) throw new Error((json as { error: string }).error);
            setData(json as GraphData);
            setLoading(false);
          }
          return;
        }

        // Priority 2: baked-in graph data (cgx publish static mode)
        if (window.__CGX_GRAPH__) {
          if (!cancelled) {
            setData(window.__CGX_GRAPH__);
            setLoading(false);
          }
          return;
        }

        // Priority 3: fetch from local API (cgx serve mode)
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
