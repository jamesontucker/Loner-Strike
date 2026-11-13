// Development only (editor-dev.html): the map editor on its own page, with the app's styles,
// toasts and questions, for trying it in a browser and taking screenshots.

import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ConfirmProvider, ToastProvider } from "../../../ui";
import "../../../styles.css";
import "../tools.css";
import "../common/common.css";
import MapEditor from "./MapEditor";

// (panels like the prefab library ask the app with react-query, as in the app)
const queries = new QueryClient();

createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={queries}>
  <ToastProvider>
    <ConfirmProvider>
      <main className="editor-dev" style={{ height: "100vh", display: "flex", flexDirection: "column", padding: 12, boxSizing: "border-box" }}>
        <MapEditor maps={[]} />
      </main>
    </ConfirmProvider>
  </ToastProvider>
  </QueryClientProvider>,
);
