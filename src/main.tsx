import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import LibraryBrowser from "./LibraryBrowser";
import { AccountGate } from "./components/AccountGate";

const view = new URLSearchParams(window.location.search).get("view");
const Root = view === "library" ? LibraryBrowser : App;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {/* The library window is only opened from the main one, so it doesn't need its own sign-in (and two windows refreshing the same session would race). */}
    {view === "library" ? (
      <Root />
    ) : (
      <AccountGate>
        <Root />
      </AccountGate>
    )}
  </React.StrictMode>,
);
