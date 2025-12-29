import ReactDOM from "react-dom/client";
import App from "./App";
import MiniWindow from "./MiniWindow";
import SpotlightWindow from "./SpotlightWindow";
import BorderOverlay from "./BorderOverlay";
import "./index.css";

const params = new URLSearchParams(window.location.search);
const isMini = params.has("mini");
const isSpotlight = params.has("spotlight");
const isBorder = params.has("border");

let Component = App;
if (isMini) Component = MiniWindow;
if (isSpotlight) Component = SpotlightWindow;
if (isBorder) Component = BorderOverlay;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <Component />
);
