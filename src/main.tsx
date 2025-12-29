import ReactDOM from "react-dom/client";
import App from "./App";
import MiniWindow from "./MiniWindow";
import SpotlightWindow from "./SpotlightWindow";
import "./index.css";

const params = new URLSearchParams(window.location.search);
const isMini = params.has("mini");
const isSpotlight = params.has("spotlight");

let Component = App;
if (isMini) Component = MiniWindow;
if (isSpotlight) Component = SpotlightWindow;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <Component />
);
