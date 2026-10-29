import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { LANG } from "./i18n";
import { applyLook } from "./settings";
import "./styles.css";

document.documentElement.lang = LANG;
applyLook();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
