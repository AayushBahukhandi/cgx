import { useState, useEffect, useRef } from "react";

interface Props {
  query: string;
  onChange: (q: string) => void;
  placeholder?: string;
}

export default function SearchBar({ query, onChange, placeholder = "Search..." }: Props) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [focused, setFocused] = useState(false);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, []);

  return (
    <div className="relative flex-1 max-w-md">
      <input
        ref={inputRef}
        type="text"
        value={query}
        onChange={(e) => onChange(e.target.value)}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        placeholder={placeholder}
        className="w-full px-3 py-1.5 text-sm outline-none"
        style={{
          background: "#0a0a0f",
          color: "#ccccdd",
          border: focused ? "1px solid #3b82f6" : "1px solid #1e1e2e",
          fontFamily: "JetBrains Mono, monospace",
          transition: "border-color 0.15s",
        }}
      />
      <span
        className="absolute right-2 top-1/2 -translate-y-1/2 text-xs pointer-events-none"
        style={{ color: "#444466", fontFamily: "JetBrains Mono, monospace" }}
      >
        {query.length > 0 ? "" : "⌘K"}
      </span>
    </div>
  );
}
