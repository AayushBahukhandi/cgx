import { useState, useEffect } from "react";
import { Highlight, themes } from "prism-react-renderer";

interface SnippetLine {
  num: number;
  text: string;
}

interface SnippetData {
  path: string;
  from: number;
  to: number;
  lines: SnippetLine[];
  language: string;
  total_lines: number;
}

interface Props {
  path: string;
  lineStart: number;
  lineEnd: number;
}

const PRISM_LANG: Record<string, string> = {
  typescript: "tsx",
  javascript: "javascript",
  python: "python",
  rust: "rust",
  go: "go",
  java: "java",
  csharp: "csharp",
  json: "json",
  markdown: "markdown",
  html: "html",
  css: "css",
  text: "text",
  c: "c",
  cpp: "cpp",
};

export default function SnippetPreview({ path, lineStart, lineEnd }: Props) {
  const [data, setData] = useState<SnippetData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const from = lineStart > 0 ? lineStart : 1;
  const rawTo = lineEnd > lineStart ? lineEnd : from + 15;
  // Show at least 8 lines of context so tiny functions don't appear as a single line
  const to = rawTo - from < 7 ? from + 7 : rawTo;

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    const url = `/api/snippet?path=${encodeURIComponent(path)}&from=${from}&to=${to}`;
    fetch(url)
      .then((resp) => {
        if (!resp.ok) {
          return resp.text().then((msg) => {
            throw new Error(msg || `HTTP ${resp.status}`);
          });
        }
        return resp.json();
      })
      .then((json: SnippetData & { error?: string }) => {
        if (!cancelled) {
          if (json.error) {
            setError(json.error);
          } else {
            setData(json);
          }
          setLoading(false);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(err.message || "Failed to load snippet");
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [path, from, to]);

  const handleOpen = async () => {
    try {
      const url = `/api/open?path=${encodeURIComponent(path)}&line=${lineStart}`;
      await fetch(url);
    } catch {
      // silently ignore fetch errors from open endpoint
    }
  };

  const prismLang = PRISM_LANG[data?.language || "text"] || "text";

  if (loading) {
    return (
      <div className="border-t" style={{ borderColor: "#1e1e2e" }}>
        <div className="px-3 py-1.5 text-xs font-bold" style={{ color: "#444466", fontFamily: "Syne, sans-serif" }}>
          SOURCE
        </div>
        <div className="px-3 pb-2">
          <div className="flex gap-1.5 mb-1">
            <div className="h-2 flex-1 rounded-sm animate-pulse" style={{ background: "#1e1e2e" }} />
            <div className="h-2 w-24 rounded-sm animate-pulse" style={{ background: "#1e1e2e" }} />
          </div>
          <div className="flex gap-1.5">
            <div className="h-2 w-16 rounded-sm animate-pulse" style={{ background: "#1e1e2e" }} />
            <div className="h-2 flex-1 rounded-sm animate-pulse" style={{ background: "#1e1e2e" }} />
          </div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="border-t" style={{ borderColor: "#1e1e2e" }}>
        <div className="px-3 py-1.5 text-xs font-bold" style={{ color: "#444466", fontFamily: "Syne, sans-serif" }}>
          SOURCE
        </div>
        <div className="px-3 pb-2">
          <p className="text-xs" style={{ color: "#553333", fontFamily: "JetBrains Mono, monospace" }}>
            {error}
          </p>
        </div>
      </div>
    );
  }

  if (!data || data.lines.length === 0) {
    return (
      <div className="border-t" style={{ borderColor: "#1e1e2e" }}>
        <div className="px-3 py-1.5 text-xs font-bold" style={{ color: "#444466", fontFamily: "Syne, sans-serif" }}>
          SOURCE
        </div>
        <div className="px-3 pb-2">
          <p className="text-xs" style={{ color: "#333350", fontFamily: "JetBrains Mono, monospace" }}>
            (no source available)
          </p>
        </div>
      </div>
    );
  }

  const code = data.lines.map((l) => l.text).join("\n");
  const firstLineNum = data.lines[0].num;
  const lastLineNum = data.lines[data.lines.length - 1].num;
  const lineDigits = String(lastLineNum).length;

  return (
    <div className="border-t" style={{ borderColor: "#1e1e2e" }}>
      {/* Header row */}
      <div className="flex items-center justify-between px-3 py-1.5">
        <span className="text-xs font-bold" style={{ color: "#444466", fontFamily: "Syne, sans-serif" }}>
          SOURCE
        </span>
        <button
          onClick={handleOpen}
          className="text-xs px-2 py-0.5 rounded-sm transition-colors hover:opacity-80"
          style={{
            color: "#8888aa",
            background: "#1a1a26",
            fontFamily: "JetBrains Mono, monospace",
          }}
          title="Open in editor"
        >
          Open
        </button>
      </div>

      {/* Snippet with line numbers */}
      <div
        className="overflow-auto font-mono text-xs leading-5"
        style={{
          maxHeight: "320px",
          background: "#0a0a10",
          fontFamily: "JetBrains Mono, monospace",
          fontSize: "0.6875rem",
        }}
      >
        <Highlight
          theme={themes.nightOwl}
          code={code}
          language={prismLang}
        >
          {({ tokens, getLineProps, getTokenProps }) => (
            <pre className="m-0 p-0 bg-transparent" style={{ minWidth: "max-content" }}>
              {tokens.map((line, i) => {
                const lineNum = firstLineNum + i;
                const lineProps = getLineProps({ line, key: i });
                return (
                  <div
                    key={i}
                    {...lineProps}
                    className="flex"
                    style={{ minHeight: "1.375rem" }}
                  >
                    {/* Line number */}
                    <span
                      className="flex-shrink-0 select-none text-right pr-3 sticky left-0"
                      style={{
                        width: `${lineDigits * 0.625 + 1.5}rem`,
                        color: "#333350",
                        userSelect: "none",
                        background: "#0a0a10",
                      }}
                    >
                      {lineNum}
                    </span>
                    {/* Code */}
                    <span className="whitespace-pre">
                      {line.map((token, j) => (
                        <span key={j} {...getTokenProps({ token, key: j })} />
                      ))}
                    </span>
                  </div>
                );
              })}
            </pre>
          )}
        </Highlight>
      </div>

      {/* Footer */}
      <div className="px-3 py-1 flex justify-between" style={{ borderTop: "1px solid #1e1e2e" }}>
        <span className="text-xs" style={{ color: "#333350", fontFamily: "JetBrains Mono, monospace" }}>
          {path.split("/").pop()}
        </span>
        <span className="text-xs" style={{ color: "#333350", fontFamily: "JetBrains Mono, monospace" }}>
          L{firstLineNum}-{lastLineNum} / {data.total_lines}
        </span>
      </div>
    </div>
  );
}
