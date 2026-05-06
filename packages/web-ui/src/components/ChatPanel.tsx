import { useState, useRef, useEffect, useCallback } from "react";
import type { GraphNode } from "../types/graph";
import { NODE_COLORS } from "../types/graph";

interface Message {
  role: "user" | "assistant";
  content: string;
  sources?: SourceNode[];
  isStreaming?: boolean;
}

interface SourceNode {
  id: string;
  name: string;
  kind: string;
  path: string;
  churn: number;
  community: number;
}

interface Props {
  graphData: { nodes: GraphNode[] } | null;
  selectedNode: GraphNode | null;
  onSelectNode: (id: string | null) => void;
}

const SUGGESTIONS = [
  "What are the riskiest files to change?",
  "Explain the architecture of this codebase",
  "Find dead code in the project",
  "What would break if I changed the auth module?",
  "Who owns the most files?",
];

export default function ChatPanel({ selectedNode, onSelectNode }: Props) {
  const [open, setOpen] = useState(false);
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const abortRef = useRef<AbortController | null>(null);

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, []);

  useEffect(() => {
    if (open) scrollToBottom();
  }, [messages, open, scrollToBottom]);

  // Pre-fill input when a node is selected
  useEffect(() => {
    if (selectedNode && open) {
      setInput(`Tell me about ${selectedNode.name}`);
    }
  }, [selectedNode, open]);

  // Keep selectedNode in a ref to avoid re-creating sendMessage callback
  const selectedNodeRef = useRef<GraphNode | null>(null);
  selectedNodeRef.current = selectedNode;

  const sendMessage = useCallback(
    async (overrideMessage?: string) => {
      const msg = (overrideMessage || input).trim();
      if (!msg || streaming) return;

      setError(null);
      setInput("");

      const history: { role: string; content: string }[] = messages
        .filter((m) => !m.isStreaming)
        .map((m) => ({ role: m.role, content: m.content }));

      const userMsg: Message = { role: "user", content: msg };
      const assistantMsg: Message = {
        role: "assistant",
        content: "",
        isStreaming: true,
      };

      setMessages((prev) => [...prev, userMsg, assistantMsg]);
      setStreaming(true);

      const controller = new AbortController();
      abortRef.current = controller;

      try {
        const resp = await fetch("/api/chat", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            message: msg,
            history,
            selected_node: selectedNodeRef.current?.id || null,
          }),
          signal: controller.signal,
        });

        if (!resp.ok) {
          const errText = await resp.text();
          throw new Error(errText || `HTTP ${resp.status}`);
        }

        const reader = resp.body?.getReader();
        if (!reader) throw new Error("No response body");

        const decoder = new TextDecoder();
        let buffer = "";
        let sources: SourceNode[] | undefined;

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          buffer += decoder.decode(value, { stream: true });

          const parts = buffer.split("\n\n");
          buffer = parts.pop() || "";

          for (const part of parts) {
            const lines = part.split("\n");
            for (const line of lines) {
              if (!line.startsWith("data: ")) continue;
              const jsonStr = line.slice(6);
              try {
                const event = JSON.parse(jsonStr);

                if (event.type === "sources") {
                  sources = event.nodes || [];
                } else if (event.type === "delta") {
                  const text = event.text || "";
                  setMessages((prev) =>
                    prev.map((m, idx) =>
                      idx === prev.length - 1 && m.isStreaming
                        ? { ...m, content: m.content + text }
                        : m
                    )
                  );
                } else if (event.type === "error") {
                  setError(event.message);
                }
              } catch {
                // Skip unparseable events
              }
            }
          }
        }

        // Finalize the last assistant message
        setMessages((prev) =>
          prev.map((m, idx) =>
            idx === prev.length - 1 && m.isStreaming
              ? { ...m, isStreaming: false, sources }
              : m
          )
        );
      } catch (err: unknown) {
        if (err instanceof DOMException && err.name === "AbortError") {
          setMessages((prev) =>
            prev.map((m, idx) =>
              idx === prev.length - 1 && m.isStreaming
                ? { ...m, isStreaming: false, content: m.content + "\n\n*[stopped]*" }
                : m
            )
          );
          return;
        }
        const errMsg = err instanceof Error ? err.message : "Request failed";
        setError(errMsg);
        setMessages((prev) =>
          prev.map((m, idx) =>
            idx === prev.length - 1 && m.isStreaming
              ? { ...m, isStreaming: false, content: `Error: ${errMsg}` }
              : m
          )
        );
      } finally {
        setStreaming(false);
        abortRef.current = null;
      }
    },
    [input, messages, streaming]
  );

  const stopStreaming = useCallback(() => {
    abortRef.current?.abort();
  }, []);

  const clearChat = useCallback(() => {
    setMessages([]);
    setError(null);
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        sendMessage();
      }
    },
    [sendMessage]
  );

  // Floating chat button
  if (!open) {
    return (
      <button
        onClick={() => setOpen(true)}
        className="fixed bottom-5 right-5 z-50 w-12 h-12 rounded-sm flex items-center justify-center shadow-lg hover:scale-105 transition-transform"
        style={{
          background: "#1a1a26",
          border: "1px solid #2a2a3e",
        }}
        title="Chat with codebase"
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#00ff88" strokeWidth="2" strokeLinecap="round">
          <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
        </svg>
      </button>
    );
  }

  return (
    <div
      className="fixed bottom-5 right-5 z-50 w-96 flex flex-col rounded-sm shadow-2xl"
      style={{
        background: "#111118",
        border: "1px solid #1e1e2e",
        maxHeight: "560px",
      }}
    >
      {/* Header */}
      <div
        className="flex items-center justify-between px-3 py-2 border-b flex-shrink-0"
        style={{ borderColor: "#1e1e2e" }}
      >
        <span
          className="text-xs font-bold tracking-wide"
          style={{ color: "#00ff88", fontFamily: "Syne, sans-serif" }}
        >
          CHAT
        </span>
        <div className="flex items-center gap-1">
          {streaming && (
            <button
              onClick={stopStreaming}
              className="w-6 h-6 flex items-center justify-center rounded-sm text-xs hover:opacity-80"
              style={{ background: "#2a1a1a", color: "#ef4444" }}
              title="Stop"
            >
              <svg width="10" height="10" viewBox="0 0 10 10" fill="#ef4444">
                <rect x="1" y="1" width="8" height="8" />
              </svg>
            </button>
          )}
          {messages.length > 0 && (
            <button
              onClick={clearChat}
              className="w-6 h-6 flex items-center justify-center rounded-sm text-xs hover:opacity-80"
              style={{ background: "#1a1a26", color: "#666688" }}
              title="Clear"
            >
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="#666688" strokeWidth="2">
                <polyline points="3 6 5 6 21 6" />
                <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
              </svg>
            </button>
          )}
          <button
            onClick={() => setOpen(false)}
            className="w-6 h-6 flex items-center justify-center rounded-sm text-xs hover:opacity-80"
            style={{ background: "#1a1a26", color: "#6666aa" }}
            title="Close"
          >
            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="#6666aa" strokeWidth="2">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
      </div>

      {/* Messages */}
      <div
        className="flex-1 overflow-y-auto px-3 py-2 space-y-3"
        style={{ maxHeight: "380px" }}
      >
        {messages.length === 0 && !error && (
          <div className="py-4 space-y-2">
            <p
              className="text-xs mb-2"
              style={{ color: "#444466", fontFamily: "JetBrains Mono, monospace" }}
            >
              Ask a question about this codebase:
            </p>
            {SUGGESTIONS.map((s, i) => (
              <button
                key={i}
                onClick={() => sendMessage(s)}
                className="w-full text-left px-2 py-1.5 rounded-sm text-xs hover:opacity-80 transition-colors truncate"
                style={{
                  background: "#1a1a26",
                  color: "#8888aa",
                  fontFamily: "JetBrains Mono, monospace",
                  border: "1px solid #252535",
                }}
              >
                {s}
              </button>
            ))}
          </div>
        )}

        {messages.map((msg, i) => (
          <div key={i}>
            {/* Role label */}
            <div
              className="text-xs font-bold mb-1"
              style={{
                color: msg.role === "user" ? "#3b82f6" : "#00ff88",
                fontFamily: "Syne, sans-serif",
              }}
            >
              {msg.role === "user" ? "YOU" : "AI"}
            </div>

            {/* Content */}
            <div
              className="text-xs leading-relaxed whitespace-pre-wrap"
              style={{
                color: msg.role === "user" ? "#aaaacc" : "#ccccdd",
                fontFamily: "JetBrains Mono, monospace",
              }}
            >
              {msg.content}
              {msg.isStreaming && (
                <span
                  className="inline-block w-2 h-4 ml-0.5 animate-pulse"
                  style={{ background: "#00ff88", verticalAlign: "text-bottom" }}
                />
              )}
            </div>

            {/* Source pills */}
            {msg.sources && msg.sources.length > 0 && !msg.isStreaming && (
              <div className="flex flex-wrap gap-1 mt-2">
                {msg.sources.slice(0, 6).map((s, j) => {
                  const color = NODE_COLORS[s.kind] || "#888888";
                  return (
                    <button
                      key={j}
                      onClick={() => onSelectNode(s.id)}
                      className="px-1.5 py-0.5 rounded-sm text-xs truncate max-w-32 hover:opacity-80 transition-opacity"
                      style={{
                        background: color + "22",
                        color,
                        border: `1px solid ${color}44`,
                        fontFamily: "JetBrains Mono, monospace",
                        fontSize: "0.625rem",
                      }}
                      title={`${s.name} (${s.kind}) — click to select`}
                    >
                      {s.name}
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        ))}

        {error && (
          <div
            className="text-xs p-2 rounded-sm"
            style={{
              background: "#2a1515",
              color: "#ef4444",
              border: "1px solid #3a2020",
              fontFamily: "JetBrains Mono, monospace",
            }}
          >
            {error}
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div className="p-2 border-t flex-shrink-0" style={{ borderColor: "#1e1e2e" }}>
        <div className="flex gap-2">
          <textarea
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Ask about the codebase..."
            rows={2}
            disabled={streaming}
            className="flex-1 resize-none px-2 py-1.5 text-xs rounded-sm outline-none"
            style={{
              background: "#0a0a10",
              color: "#ccccdd",
              fontFamily: "JetBrains Mono, monospace",
              border: "1px solid #252535",
              minHeight: "36px",
            }}
          />
          <button
            onClick={() => sendMessage()}
            disabled={streaming || !input.trim()}
            className="px-3 rounded-sm text-xs font-bold transition-opacity"
            style={{
              background: input.trim() && !streaming ? "#00ff8822" : "#1a1a26",
              color: input.trim() && !streaming ? "#00ff88" : "#444466",
              border: `1px solid ${input.trim() && !streaming ? "#00ff8844" : "#252535"}`,
              fontFamily: "Syne, sans-serif",
            }}
          >
            Send
          </button>
        </div>
      </div>
    </div>
  );
}
