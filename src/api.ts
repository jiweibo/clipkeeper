import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Clip = {
  id: number;
  kind: "text" | "image";
  text: string;
  imagePath: string | null;
  width: number | null;
  height: number | null;
  pinned: boolean;
  createdAt: number;
};

export type Theme = "system" | "light" | "dark";
export type Accent = "blue" | "purple" | "green" | "orange" | "pink" | "graphite";

export type Settings = {
  maxItems: number;
  theme: Theme;
  accent: Accent;
  shortcut: string;
  saveImages: boolean;
};

/** Settings plus the launch-at-login flag, which the OS keeps. */
export type SettingsView = Settings & { launchAtLogin: boolean };

export const getSettings = () => invoke<SettingsView>("get_settings");
export const saveSettings = (settings: Settings) =>
  invoke<SettingsView>("save_settings", { settings });
export const setLaunchAtLogin = (enabled: boolean) =>
  invoke<SettingsView>("set_launch_at_login", { enabled });
export const pauseShortcut = (paused: boolean) => invoke<void>("pause_shortcut", { paused });

export const listClips = (query: string) => invoke<Clip[]>("list_clips", { query, limit: 300 });
export const copyClip = (id: number) => invoke<void>("copy_clip", { id });
export const setPinned = (id: number, pinned: boolean) => invoke<void>("set_pinned", { id, pinned });
export const deleteClip = (id: number) => invoke<void>("delete_clip", { id });
export const clearHistory = () => invoke<void>("clear_history");

export const hideWindow = () => invoke<void>("hide_window");

export const onClipsChanged = (cb: () => void) => listen("clips-changed", cb);

export const onWindowShown = (cb: () => void) => listen("window-shown", cb);

export const onOpenSettings = (cb: () => void) => listen("open-settings", cb);
export const onSettingsChanged = (cb: (s: SettingsView) => void) =>
  listen<SettingsView>("settings-changed", (e) => cb(e.payload));

export const imageSrc = (clip: Clip) => (clip.imagePath ? convertFileSrc(clip.imagePath) : "");
