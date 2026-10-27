import { useEffect, useState } from "react";
import {
  Accent,
  getSettings,
  pauseShortcut,
  saveSettings,
  setLaunchAtLogin,
  Settings as SettingsData,
  SettingsView,
  Theme,
} from "./api";

const isMac = navigator.userAgent.includes("Mac");

const DEFAULT_SHORTCUT = "CmdOrCtrl+Shift+V";
const ITEM_OPTIONS = [100, 200, 500, 1000, 2000, 5000, 10000];

const THEMES: { key: Theme; label: string }[] = [
  { key: "system", label: "跟随系统" },
  { key: "light", label: "浅色" },
  { key: "dark", label: "深色" },
];

const ACCENTS: { key: Accent; label: string }[] = [
  { key: "blue", label: "蓝" },
  { key: "purple", label: "紫" },
  { key: "green", label: "绿" },
  { key: "orange", label: "橙" },
  { key: "pink", label: "粉" },
  { key: "graphite", label: "石墨" },
];

/** Applies theme and accent colour by tagging the root element; the CSS
 * picks colours from these attributes. */
export function applyAppearance(s: SettingsView) {
  document.documentElement.dataset.theme = s.theme;
  document.documentElement.dataset.accent = s.accent;
}

/** Same rules as `shortcut_label` in settings.rs. */
export function shortcutLabel(shortcut: string): string {
  const names: Record<string, [string, string]> = {
    cmdorctrl: ["⌘", "Ctrl"],
    commandorcontrol: ["⌘", "Ctrl"],
    cmd: ["⌘", "Win"],
    command: ["⌘", "Win"],
    super: ["⌘", "Win"],
    ctrl: ["⌃", "Ctrl"],
    control: ["⌃", "Ctrl"],
    alt: ["⌥", "Alt"],
    option: ["⌥", "Alt"],
    shift: ["⇧", "Shift"],
  };
  const parts = shortcut.split("+").map((t) => {
    const n = names[t.toLowerCase()];
    if (n) return isMac ? n[0] : n[1];
    return t.replace(/^(Key|Digit)/, "");
  });
  return parts.join(isMac ? "" : "+");
}

const KEY_CODE =
  /^(Key[A-Z]|Digit\d|F\d{1,2}|Space|Backquote|Minus|Equal|BracketLeft|BracketRight|Backslash|Semicolon|Quote|Comma|Period|Slash)$/;

/** Builds a shortcut string from a key press, or null if the press isn't a
 * usable global shortcut yet (only modifiers, or no Ctrl/Alt/⌘). */
function comboFromEvent(e: React.KeyboardEvent): string | null {
  if (!KEY_CODE.test(e.code)) return null;
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push(isMac ? "Cmd" : "Super");
  const strong = mods.filter((m) => m !== "Shift").length > 0;
  if (!strong && !/^F\d/.test(e.code)) return null;
  return [...mods, e.code].join("+");
}

function Toggle({ on, onChange }: { on: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      role="switch"
      aria-checked={on}
      className={on ? "switch on" : "switch"}
      onClick={() => onChange(!on)}
    >
      <span />
    </button>
  );
}

export function Settings({ onBack }: { onBack: () => void }) {
  const [settings, setSettings] = useState<SettingsView | null>(null);
  const [error, setError] = useState("");
  const [recording, setRecording] = useState(false);

  useEffect(() => {
    getSettings().then(setSettings).catch((e) => setError(String(e)));
  }, []);

  // Never leave the global shortcut switched off.
  useEffect(() => {
    if (!recording) return;
    pauseShortcut(true);
    return () => {
      pauseShortcut(false);
    };
  }, [recording]);

  if (!settings) return <div className="settings" />;

  const update = async (patch: Partial<SettingsData>) => {
    const { launchAtLogin: _, ...current } = settings;
    try {
      const saved = await saveSettings({ ...current, ...patch });
      setSettings(saved);
      applyAppearance(saved);
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };

  const toggleLaunch = async (on: boolean) => {
    try {
      setSettings(await setLaunchAtLogin(on));
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (recording) {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setRecording(false);
        return;
      }
      const combo = comboFromEvent(e);
      if (combo) {
        setRecording(false);
        update({ shortcut: combo });
      }
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      onBack();
    }
  };

  const itemOptions = ITEM_OPTIONS.includes(settings.maxItems)
    ? ITEM_OPTIONS
    : [...ITEM_OPTIONS, settings.maxItems].sort((a, b) => a - b);

  return (
    <div className="settings" onKeyDown={onKeyDown} tabIndex={-1}>
      <header className="settings-top">
        <button className="back" onClick={onBack} autoFocus>
          <svg viewBox="0 0 20 20">
            <path d="M12.5 4.5L7 10l5.5 5.5" />
          </svg>
          返回
        </button>
        <h1>设置</h1>
      </header>

      <div className="settings-body">
        {error && <div className="settings-error">{error}</div>}

        <section>
          <h2>外观</h2>
          <div className="row">
            <span>主题</span>
            <div className="segmented">
              {THEMES.map((t) => (
                <button
                  key={t.key}
                  className={settings.theme === t.key ? "active" : ""}
                  onClick={() => update({ theme: t.key })}
                >
                  {t.label}
                </button>
              ))}
            </div>
          </div>
          <div className="row">
            <span>皮肤颜色</span>
            <div className="swatches">
              {ACCENTS.map((a) => (
                <button
                  key={a.key}
                  title={a.label}
                  aria-label={a.label}
                  data-swatch={a.key}
                  className={settings.accent === a.key ? "swatch active" : "swatch"}
                  onClick={() => update({ accent: a.key })}
                />
              ))}
            </div>
          </div>
        </section>

        <section>
          <h2>历史记录</h2>
          <div className="row">
            <span>
              最多保存
              <small>超出后自动删除最早的记录，置顶的不受影响</small>
            </span>
            <select
              value={settings.maxItems}
              onChange={(e) => update({ maxItems: Number(e.target.value) })}
            >
              {itemOptions.map((n) => (
                <option key={n} value={n}>
                  {n} 条
                </option>
              ))}
            </select>
          </div>
          <div className="row">
            <span>
              保存图片
              <small>关闭后只记录文字</small>
            </span>
            <Toggle on={settings.saveImages} onChange={(v) => update({ saveImages: v })} />
          </div>
        </section>

        <section>
          <h2>快捷键</h2>
          <div className="row">
            <span>打开剪贴板历史</span>
            <div className="shortcut-edit">
              <button
                className={recording ? "recorder recording" : "recorder"}
                onClick={() => setRecording((r) => !r)}
              >
                {recording ? "请按下新的组合键…" : shortcutLabel(settings.shortcut)}
              </button>
              {settings.shortcut !== DEFAULT_SHORTCUT && !recording && (
                <button className="link" onClick={() => update({ shortcut: DEFAULT_SHORTCUT })}>
                  恢复默认
                </button>
              )}
            </div>
          </div>
          {recording && (
            <p className="hint">
              需要包含 {isMac ? "⌘、⌃ 或 ⌥" : "Ctrl、Alt 或 Win"} 中的一个，按 Esc 取消
            </p>
          )}
        </section>

        <section>
          <h2>系统</h2>
          <div className="row">
            <span>开机自动启动</span>
            <Toggle on={settings.launchAtLogin} onChange={toggleLaunch} />
          </div>
        </section>
      </div>
    </div>
  );
}
