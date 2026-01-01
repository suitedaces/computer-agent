import ReactDOM from "react-dom/client";
import MainWindow from "./MainWindow";
import VoiceWindow from "./VoiceWindow";
import BorderOverlay from "./BorderOverlay";
import "./index.css";

const params = new URLSearchParams(window.location.search);
const isVoice = params.has("voice");
const isBorder = params.has("border");

let Component = MainWindow;
if (isVoice) Component = VoiceWindow;
if (isBorder) Component = BorderOverlay;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <Component />
);
