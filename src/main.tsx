import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import "@fontsource-variable/bricolage-grotesque/opsz.css";
import "./App.css";
import "./widget/widget.css";
import { Widget } from "./widget/Widget";

// A mesma página serve as duas janelas: a principal e a barrinha de uso.
const isWidget = getCurrentWindow().label === "widget";
if (isWidget) document.documentElement.classList.add("widget");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isWidget ? <Widget /> : <App />}</React.StrictMode>,
);
