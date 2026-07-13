import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { unavailableAccountService } from "./account/accountService";

let accountService = unavailableAccountService;

if (import.meta.env.VITE_SUPABASE_URL && import.meta.env.VITE_SUPABASE_PUBLISHABLE_KEY) {
  const { SupabaseAccountService } = await import("./account/supabaseAccountService");
  accountService = new SupabaseAccountService(
    import.meta.env.VITE_SUPABASE_URL,
    import.meta.env.VITE_SUPABASE_PUBLISHABLE_KEY,
  );
}

if (import.meta.env.MODE === "e2e") {
  await import("@wdio/tauri-plugin");
  accountService = (await import("./account/e2eAccountService")).e2eAccountService;
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App accountService={accountService} />
  </StrictMode>,
);
