import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type Clip = {
  id: number;
  kind: "text" | "image";
  text: string;
  imagePath: string | null;
  width: number | null;
  height: number | null;
  pinned: boolean;
  createdAt: number;
};

// Placeholder screen; the real popup UI comes in step 3.
function App() {
  const [clips, setClips] = useState<Clip[]>([]);

  useEffect(() => {
    invoke<Clip[]>("list_clips", { query: "" }).then(setClips).catch(console.error);
  }, []);

  return (
    <main className="container">
      <h1>ClipKeeper</h1>
      <p>{clips.length} 条历史记录</p>
    </main>
  );
}

export default App;
