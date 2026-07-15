import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { unavailableAccountService } from "./account/accountService";
import { tauriTrustedDeviceService } from "./trusted-device/tauriTrustedDeviceService";
import { tauriCabinetService } from "./cabinet/cabinetService";
import { tauriConversationService } from "./conversation/conversationService";

let accountService = unavailableAccountService;
let trustedDeviceService = tauriTrustedDeviceService;

if (import.meta.env.VITE_SUPABASE_URL && import.meta.env.VITE_SUPABASE_PUBLISHABLE_KEY) {
  const { SupabaseAccountService } = await import("./account/supabaseAccountService");
  const { tauriAccountSessionStorage } = await import("./account/tauriAccountSessionStorage");
  accountService = new SupabaseAccountService(
    import.meta.env.VITE_SUPABASE_URL,
    import.meta.env.VITE_SUPABASE_PUBLISHABLE_KEY,
    tauriAccountSessionStorage,
  );
}

if (import.meta.env.MODE === "e2e") {
  await import("@wdio/tauri-plugin");
  const e2eAccount = await import("./account/e2eAccountService");
  accountService = e2eAccount.e2eAccountService;
  Object.defineProperty(window, "__LUNA_E2E_ACCOUNT__", {
    value: e2eAccount.e2eAccountTestControl,
  });
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App
      accountService={accountService}
      cabinetService={tauriCabinetService}
      conversationService={tauriConversationService}
      trustedDeviceService={trustedDeviceService}
    />
  </StrictMode>,
);
