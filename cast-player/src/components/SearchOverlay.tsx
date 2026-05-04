import { useState, useCallback, useEffect, useRef } from "react";
import type { SearchAddon } from "@xterm/addon-search";
import { useTranslation } from "../i18n";

interface SearchOverlayProps {
  searchAddonRef: React.MutableRefObject<SearchAddon | null>;
  visible: boolean;
  onClose: () => void;
}

/**
 * 终端内查找浮层. 用 @xterm/addon-search 提供的 findNext / findPrevious,
 * 自动滚动 + decoration 高亮匹配 (替换之前手撸的 buffer 扫描, 那个不会高亮
 * 也不准确滚动).
 */
export function SearchOverlay({ searchAddonRef, visible, onClose }: SearchOverlayProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [regex, setRegex] = useState(false);
  // count: 当前查询命中总数; current: 第几个 (1-based, 0 = 无 query / 无 hit)
  const [count, setCount] = useState(0);
  const [current, setCurrent] = useState(0);
  // SearchAddon 触发的 onDidChangeResults 报告 (resultIndex, resultCount)
  // 把 listener 注册到 ref 上, visible 切回 true 时复用同一 SearchAddon 实例
  const listenerInstalledRef = useRef<SearchAddon | null>(null);

  useEffect(() => {
    const addon = searchAddonRef.current;
    if (!addon || listenerInstalledRef.current === addon) return;
    addon.onDidChangeResults(({ resultIndex, resultCount }) => {
      setCount(resultCount);
      // resultIndex < 0 表示 "在 buffer 内但未定位到具体一个" → 当 0 显示
      setCurrent(resultIndex >= 0 ? resultIndex + 1 : 0);
    });
    listenerInstalledRef.current = addon;
  }, [searchAddonRef, visible]);

  // 输入变化时立即 findNext 触发首次定位
  const runSearch = useCallback(
    (q: string, cs: boolean, re: boolean) => {
      const addon = searchAddonRef.current;
      if (!addon) return;
      if (!q) {
        addon.clearDecorations();
        setCount(0);
        setCurrent(0);
        return;
      }
      addon.findNext(q, {
        caseSensitive: cs,
        regex: re,
        wholeWord: false,
        decorations: {
          matchBackground: "#FFA500",
          matchBorder: "#FFFFFF",
          matchOverviewRuler: "#FFA500",
          activeMatchBackground: "#FF6B00",
          activeMatchBorder: "#FFFFFF",
          activeMatchColorOverviewRuler: "#FF6B00",
        },
      });
    },
    [searchAddonRef]
  );

  const handleSearch = (q: string) => {
    setQuery(q);
    runSearch(q, caseSensitive, regex);
  };

  const navigateMatch = (dir: 1 | -1) => {
    const addon = searchAddonRef.current;
    if (!addon || !query) return;
    const opts = {
      caseSensitive,
      regex,
      wholeWord: false,
      // 复用首次 findNext 设置的 decorations, 此处不必重传
    };
    if (dir === 1) addon.findNext(query, opts);
    else addon.findPrevious(query, opts);
  };

  // visible 切 false 时清掉 decoration, 否则用户关浮层后高亮还在
  useEffect(() => {
    if (!visible) {
      searchAddonRef.current?.clearDecorations();
    }
  }, [visible, searchAddonRef]);

  if (!visible) return null;

  return (
    <div className="search-overlay">
      <div className="search-row">
        <input
          type="text"
          placeholder={t("search.placeholder")}
          value={query}
          onChange={(e) => handleSearch(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              navigateMatch(e.shiftKey ? -1 : 1);
              e.preventDefault();
            } else if (e.key === "Escape") {
              onClose();
            }
          }}
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
          {count > 0 ? `${current}/${count}` : query ? t("search.noMatch") : ""}
        </span>
        <div className="search-options">
          <label>
            <input
              type="checkbox"
              checked={caseSensitive}
              onChange={(e) => {
                setCaseSensitive(e.target.checked);
                if (query) runSearch(query, e.target.checked, regex);
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
                if (query) runSearch(query, caseSensitive, e.target.checked);
              }}
            />
            .*
          </label>
        </div>
      </div>
    </div>
  );
}
