import { useEffect } from "react";

interface KeyCallbacks {
  onEscape?: () => void;
  onEnter?: () => void;
  onArrowUp?: () => void;
  onArrowDown?: () => void;
  onCmdK?: () => void;
}

export function useKeyboard(callbacks: KeyCallbacks) {
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        callbacks.onCmdK?.();
        return;
      }

      switch (e.key) {
        case "Escape":
          callbacks.onEscape?.();
          break;
        case "Enter":
          callbacks.onEnter?.();
          break;
        case "ArrowUp":
          callbacks.onArrowUp?.();
          break;
        case "ArrowDown":
          callbacks.onArrowDown?.();
          break;
      }
    };

    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [callbacks]);
}
