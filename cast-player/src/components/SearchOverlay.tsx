import { useState, useCallback } from "react";
import { Terminal } from "@xterm/xterm";

interface SearchOverlayProps {
  termRef: React.MutableRefObject<Terminal | null>;
  visible: boolean;
  onClose: () => void;
}

export function SearchOverlay({ termRef, visible, onClose }: SearchOverlayProps) {
  const [query, setQuery] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [regex, setRegex] = useState(false);
  const [count, setCount] = useState(0);
  const [current, setCurrent] = useState(0);

  const doSearch = useCallback(
    (q: string, cs: boolean, re: boolean) => {
      const term = termRef.current;
      if (!term || !q) {
        setCount(0);
        setCurrent(0);
        return;
      }

      const bufferLines: string[] = [];
      const buffer = term.buffer.active;
      const totalLines = buffer.length;
      for (let i = 0; i < totalLines; i++) {
        const line = buffer.getLine(i);
        if (line) {
          bufferLines.push(line.translateToString(true));
        }
      }

      const fullText = bufferLines.join("\n");
      let pattern: RegExp;
      const flags = cs ? "g" : "gi";
      try {
        if (re) {
          pattern = new RegExp(q, flags);
        } else {
          pattern = new RegExp(
            q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"),
            flags
          );
        }
      } catch {
        setCount(0);
        setCurrent(0);
        return;
      }

      const matches: string[] = [...(fullText.match(pattern) ?? [])];
      setCount(matches.length);
      setCurrent(0);

      if (matches.length > 0 && matches[0]) {
        const idx = fullText.indexOf(matches[0]);
        const before = fullText.slice(0, idx);
        const lineNum = before.split("\n").length - 1;
        term.scrollToLine(Math.max(0, lineNum - 5));
      }
    },
    [termRef]
  );

  const handleSearch = (q: string) => {
    setQuery(q);
    doSearch(q, caseSensitive, regex);
  };

  // 简化的 next/prev: 在当前 buffer 中搜索并滚动
  const navigateMatch = (dir: 1 | -1) => {
    if (count === 0) return;
    const next = ((current + dir) % count + count) % count;
    setCurrent(next);

    const term = termRef.current;
    if (!term || !query) return;

    const bufferLines: string[] = [];
    const buffer = term.buffer.active;
    const totalLines = buffer.length;
    for (let i = 0; i < totalLines; i++) {
      const line = buffer.getLine(i);
      if (line) {
        bufferLines.push(line.translateToString(true));
      }
    }

    const fullText = bufferLines.join("\n");
    let pattern: RegExp;
    const flags = caseSensitive ? "g" : "gi";
    try {
      if (regex) {
        pattern = new RegExp(query, flags);
      } else {
        pattern = new RegExp(
          query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"),
          flags
        );
      }
    } catch {
      return;
    }

    const matches = fullText.match(pattern) || [];
    const hit = matches[next];
    if (hit) {
      const idx = fullText.indexOf(hit);
      const before = fullText.slice(0, idx);
      const lineNum = before.split("\n").length - 1;
      term.scrollToLine(Math.max(0, lineNum - 5));
    }
  };

  if (!visible) return null;

  return (
    <div className="search-overlay">
      <div className="search-row">
        <input
          type="text"
          placeholder="在终端中搜索..."
          value={query}
          onChange={(e) => handleSearch(e.target.value)}
          autoFocus
        />
        <button onClick={() => navigateMatch(-1)} disabled={count === 0}>
          ▲
        </button>
        <button onClick={() => navigateMatch(1)} disabled={count === 0}>
          ▼
        </button>
        <button onClick={onClose}>✕</button>
      </div>
      <div className="search-row">
        <span className="search-count">
          {count > 0 ? `${current + 1}/${count}` : "无匹配"}
        </span>
        <div className="search-options">
          <label>
            <input
              type="checkbox"
              checked={caseSensitive}
              onChange={(e) => {
                setCaseSensitive(e.target.checked);
                if (query) doSearch(query, e.target.checked, regex);
              }}
            />
            Aa
          </label>
          <label>
            <input
              type="checkbox"
              checked={regex}
              onChange={(e) => {
                setRegex(e.target.checked);
                if (query) doSearch(query, caseSensitive, e.target.checked);
              }}
            />
            .*
          </label>
        </div>
      </div>
    </div>
  );
}