import { createRoot } from "react-dom/client";
import "@fontsource/nunito/400.css";
import "@fontsource/nunito/600.css";
import "@fontsource/nunito/700.css";
import "@fontsource/nunito/800.css";
import "@fontsource/jetbrains-mono/400.css";
import "./styles/base.css";
import IslandApp from "./island/IslandApp";

document.documentElement.dataset.window = "mascot";

createRoot(document.getElementById("root")!).render(<IslandApp />);
