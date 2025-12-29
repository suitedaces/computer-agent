export default function BorderOverlay() {
  return (
    <div className="fixed inset-0 pointer-events-none overflow-hidden">
      <div className="edge-glow edge-top" />
      <div className="edge-glow edge-bottom" />
      <div className="edge-glow edge-left" />
      <div className="edge-glow edge-right" />
    </div>
  );
}
