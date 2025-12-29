import ReactDOM from "react-dom/client";
import App from "./App";
import MiniWindow from "./MiniWindow";
import "./index.css";

const isMini = new URLSearchParams(window.location.search).has("mini");
console.log("[main.tsx] isMini:", isMini, "location:", window.location.href);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  isMini ? <MiniWindow /> : <App />
);
