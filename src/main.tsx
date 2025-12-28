import ReactDOM from "react-dom/client";
import App from "./App";
import MiniWindow from "./MiniWindow";
import "./index.css";

const isMini = new URLSearchParams(window.location.search).has("mini");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  isMini ? <MiniWindow /> : <App />
);
