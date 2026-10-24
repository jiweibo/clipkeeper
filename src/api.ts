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

export const listClips = (query: string) => invoke<Clip[]>("list_clips", { query, limit: 300 });
export const copyClip = (id: number) => invoke<void>("copy_clip", { id });
export const setPinned = (id: number, pinned: boolean) => invoke<void>("set_pinned", { id, pinned });
export const deleteClip = (id: number) => invoke<void>("delete_clip", { id });
export const clearHistory = () => invoke<void>("clear_history");

export const onClipsChanged = (cb: () => void) => listen("clips-changed", cb);

export const imageSrc = (clip: Clip) => (clip.imagePath ? convertFileSrc(clip.imagePath) : "");
