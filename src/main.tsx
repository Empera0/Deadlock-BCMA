import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { LANG } from "./i18n";
import { applyLook } from "./settings";
import "@fontsource/barlow-condensed/600.css";
import "@fontsource/barlow-condensed/700.css";
import "@fontsource/zilla-slab/700.css";
import "@fontsource/alfa-slab-one/400.css";
import "@fontsource/fraunces/800.css";
import "@fontsource/big-shoulders-stencil-display/800.css";
import "./styles.css";

document.documentElement.lang = LANG;
applyLook();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
