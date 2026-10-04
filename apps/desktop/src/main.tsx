import React from "react";
import ReactDOM from "react-dom/client";

import { installViewportLock } from "@novamail/ui";

import App from "./App";
import "./styles.css";

installViewportLock();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
