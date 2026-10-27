import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { applyAppearance, Settings } from "./Settings";
import {
  Clip,
  clearHistory,
  copyClip,
  deleteClip,
  hideWindow,
  imageSrc,
  listClips,
  onClipsChanged,
  onOpenSettings,
  onSettingsChanged,
  onWindowShown,
  getSettings,
  setPinned,
} from "./api";
import "./App.css";

type Filter = "all" | "text" | "image" | "pinned";

const FILTERS: { key: Filter; label: string }[] = [
  { key: "all", label: "全部" },
  { key: "text", label: "文本" },
  { key: "image", label: "图片" },
  { key: "pinned", label: "置顶" },
];

const isMac = navigator.userAgent.includes("Mac");
const MOD = isMac ? "⌘" : "Ctrl";

function timeAgo(ms: number): string {
  const diff = Date.now() - ms;
  const min = Math.floor(diff / 60_000);
  if (min < 1) return "刚刚";
  if (min < 60) return `${min} 分钟前`;
  const hours = Math.floor(min / 60);
  if (hours < 24) return `${hours} 小时前`;
  const d = new Date(ms);
  const yesterday = new Date();
  yesterday.setDate(yesterday.getDate() - 1);
  if (d.toDateString() === yesterday.toDateString()) return "昨天";
  return `${d.getMonth() + 1}月${d.getDate()}日`;
}

function preview(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

/** Renders a text preview with the search match highlighted, starting the
 * preview near the match so it stays visible in a two-line row. */
function Highlighted({ text, query }: { text: string; query: string }) {
  const flat = preview(text);
  const q = query.trim();
  const at = q ? flat.toLowerCase().indexOf(q.toLowerCase()) : -1;
  if (at < 0) return <>{flat}</>;
  const start = at > 30 ? at - 12 : 0;
  return (
    <>
      {start > 0 && "…"}
      {flat.slice(start, at)}
      <mark>{flat.slice(at, at + q.length)}</mark>
      {flat.slice(at + q.length)}
    </>
  );
}

function History({ onOpenSettings }: { onOpenSettings: () => void }) {
  const [clips, setClips] = useState<Clip[]>([]);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [selected, setSelected] = useState(0);
  const [toast, setToast] = useState("");
  const [confirmClear, setConfirmClear] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const toastTimer = useRef<number>(undefined);

  const refresh = useCallback(() => {
    listClips(query).then(setClips).catch(console.error);
  }, [query]);

  useEffect(() => {
    const t = window.setTimeout(refresh, 120);
    return () => window.clearTimeout(t);
  }, [refresh]);

  useEffect(() => {
    const unlisten = onClipsChanged(refresh);
    return () => {
      unlisten.then((f) => f());
    };
  }, [refresh]);

  // Every time the window pops up, start fresh at the top of the list.
  useEffect(() => {
    const unlisten = onWindowShown(() => {
      setQuery("");
      setFilter("all");
      setSelected(0);
      listRef.current?.scrollTo({ top: 0 });
      searchRef.current?.focus();
      listClips("").then(setClips).catch(console.error);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const visible = useMemo(
    () =>
      clips.filter((c) =>
        filter === "all" ? true : filter === "pinned" ? c.pinned : c.kind === filter,
      ),
    [clips, filter],
  );

  useEffect(() => setSelected(0), [query, filter]);
  useEffect(() => {
    if (selected >= visible.length) setSelected(Math.max(0, visible.length - 1));
  }, [visible.length, selected]);

  useEffect(() => {
    listRef.current
      ?.querySelector<HTMLElement>(`[data-index="${selected}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const flash = (msg: string) => {
    setToast(msg);
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(""), 1400);
  };

  const copy = async (clip?: Clip) => {
    if (!clip) return;
    try {
      await copyClip(clip.id);
      // Back to the previous app, ready for Ctrl/⌘+V.
      await hideWindow();
    } catch (e) {
      flash(`复制失败：${e}`);
    }
  };

  const togglePin = async (clip?: Clip) => {
    if (!clip) return;
    await setPinned(clip.id, !clip.pinned);
    refresh();
  };

  const remove = async (clip?: Clip) => {
    if (!clip) return;
    await deleteClip(clip.id);
    refresh();
  };

  // First click arms the button, second click within 3s clears. A native
  // confirm dialog would take focus and dismiss the popup.
  useEffect(() => {
    if (!confirmClear) return;
    const t = window.setTimeout(() => setConfirmClear(false), 3000);
    return () => window.clearTimeout(t);
  }, [confirmClear]);

  const clearAll = async () => {
    if (!confirmClear) {
      setConfirmClear(true);
      return;
    }
    setConfirmClear(false);
    await clearHistory();
    refresh();
    flash("已清空未置顶的记录");
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    const current = visible[selected];
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected((i) => Math.min(i + 1, visible.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      copy(current);
    } else if (mod && e.key.toLowerCase() === "p") {
      e.preventDefault();
      togglePin(current);
    } else if (mod && (e.key === "Backspace" || e.key === "Delete")) {
      e.preventDefault();
      remove(current);
    } else if (mod && /^[1-9]$/.test(e.key)) {
      e.preventDefault();
      copy(visible[Number(e.key) - 1]);
    } else if (e.key === "Tab") {
      e.preventDefault();
      const i = FILTERS.findIndex((f) => f.key === filter);
      const next = (i + (e.shiftKey ? FILTERS.length - 1 : 1)) % FILTERS.length;
      setFilter(FILTERS[next].key);
    } else if (mod && e.key === ",") {
      e.preventDefault();
      onOpenSettings();
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (query) setQuery("");
      else hideWindow();
    }
  };

  return (
    <div className="app" onKeyDown={onKeyDown}>
      <header className="top">
        <div className="search">
          <svg viewBox="0 0 20 20" aria-hidden="true">
            <circle cx="9" cy="9" r="6" />
            <path d="M13.5 13.5 L17 17" />
          </svg>
          <input
            ref={searchRef}
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="搜索剪贴板历史"
            spellCheck={false}
          />
          {query && (
            <button className="clear-q" onClick={() => setQuery("")} aria-label="清除搜索">
              ×
            </button>
          )}
        </div>
        <nav className="tabs">
          {FILTERS.map((f) => (
            <button
              key={f.key}
              className={f.key === filter ? "tab active" : "tab"}
              onClick={() => {
                setFilter(f.key);
                searchRef.current?.focus();
              }}
            >
              {f.label}
            </button>
          ))}
          <span className="spacer" />
          <button className={confirmClear ? "link armed" : "link"} onClick={clearAll}>
            {confirmClear ? "再点一次确认清空" : "清空"}
          </button>
          <button className="icon gear" title={`设置 (${MOD},)`} onClick={onOpenSettings}>
            <svg viewBox="0 0 20 20">
              <path d="M3 6h8M15 6h2M3 14h2M9 14h8" />
              <circle cx="13" cy="6" r="2" />
              <circle cx="7" cy="14" r="2" />
            </svg>
          </button>
        </nav>
      </header>

      {visible.length === 0 ? (
        <div className="empty">
          <div className="empty-icon">📋</div>
          <p>{query ? "没有找到匹配的内容" : "还没有记录，复制点什么试试吧"}</p>
        </div>
      ) : (
        <ul className="list" ref={listRef}>
          {visible.map((clip, i) => (
            <li
              key={clip.id}
              data-index={i}
              className={i === selected ? "item selected" : "item"}
              onMouseMove={() => i !== selected && setSelected(i)}
              onClick={() => copy(clip)}
            >
              {clip.kind === "image" ? (
                <div className="thumb">
                  <img src={imageSrc(clip)} alt="" loading="lazy" />
                </div>
              ) : null}
              <div className="body">
                <div className={clip.kind === "image" ? "text meta-text" : "text"}>
                  {clip.kind === "image" ? (
                    `图片 ${clip.width} × ${clip.height}`
                  ) : (
                    <Highlighted text={clip.text} query={query} />
                  )}
                </div>
                <div className="meta">
                  {clip.pinned && <span className="pin-badge">置顶</span>}
                  <span>{timeAgo(clip.createdAt)}</span>
                  {clip.kind === "text" && clip.text.length > 80 && (
                    <span>{clip.text.length} 字</span>
                  )}
                </div>
              </div>
              <div className="actions">
                {i < 9 && <kbd className="shortcut">{MOD}{i + 1}</kbd>}
                <button
                  className={clip.pinned ? "icon on" : "icon"}
                  title={clip.pinned ? "取消置顶" : "置顶"}
                  onClick={(e) => {
                    e.stopPropagation();
                    togglePin(clip);
                  }}
                >
                  <svg viewBox="0 0 20 20">
                    <path d="M7 3h6l-1 5 3 3H5l3-3z M10 11v6" />
                  </svg>
                </button>
                <button
                  className="icon danger"
                  title="删除"
                  onClick={(e) => {
                    e.stopPropagation();
                    remove(clip);
                  }}
                >
                  <svg viewBox="0 0 20 20">
                    <path d="M4 6h12 M8 6V4h4v2 M6 6l1 10h6l1-10" />
                  </svg>
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      <footer className="hints">
        <span><kbd>↑</kbd><kbd>↓</kbd> 选择</span>
        <span><kbd>Enter</kbd> 复制</span>
        <span><kbd>{MOD}</kbd><kbd>P</kbd> 置顶</span>
        <span><kbd>{MOD}</kbd><kbd>⌫</kbd> 删除</span>
      </footer>

      <div className={toast ? "toast show" : "toast"}>{toast}</div>
    </div>
  );
}

function App() {
  const [view, setView] = useState<"history" | "settings">("history");

  useEffect(() => {
    getSettings().then(applyAppearance).catch(console.error);
    const subs = [
      onSettingsChanged(applyAppearance),
      onWindowShown(() => setView("history")),
      onOpenSettings(() => setView("settings")),
    ];
    return () => {
      subs.forEach((p) => p.then((f) => f()));
    };
  }, []);

  return view === "settings" ? (
    <Settings onBack={() => setView("history")} />
  ) : (
    <History onOpenSettings={() => setView("settings")} />
  );
}

export default App;
