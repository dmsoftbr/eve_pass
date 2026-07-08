import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { Palette } from "./components/Palette";
import { VaultProvider } from "./state/vault";
import "./styles.css";

// Two windows load this same bundle. The frameless "palette" window renders its
// own lightweight UI; everything else is the main app.
const isPalette = (() => {
  try {
    return getCurrentWindow().label === "palette";
  } catch {
    return false;
  }
})();

const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

root.render(
  <React.StrictMode>
    {isPalette ? (
      <Palette />
    ) : (
      <VaultProvider>
        <App />
      </VaultProvider>
    )}
  </React.StrictMode>,
);
