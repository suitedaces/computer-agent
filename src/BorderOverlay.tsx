import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

export default function BorderOverlay() {
  useEffect(() => {
    // listen for agent state changes to potentially adjust animation
    const unlisten = listen("agent:stopped", () => {
      // window will be hidden by rust, but we could do cleanup here
    });
    return () => { unlisten.then((f) => f()); };
  }, []);

  return (
    <div className="fixed inset-0 pointer-events-none">
      <div className="absolute inset-0 border-overlay" />
    </div>
  );
}
