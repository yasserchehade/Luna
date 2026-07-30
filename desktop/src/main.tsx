import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { unavailableAccountService } from "./account/accountService";
import { tauriTrustedDeviceService } from "./trusted-device/tauriTrustedDeviceService";
import { tauriCabinetService } from "./cabinet/cabinetService";
import { tauriConversationService } from "./conversation/conversationService";
import { synchronizeManagedIntelligenceAccess } from "./account/managedIntelligenceCoordinator";

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

if (import.meta.env.MODE === "e2e" || import.meta.env.MODE === "live-canary") {
  await import("@wdio/tauri-plugin");
}

if (import.meta.env.MODE === "e2e") {
  const e2eAccount = await import("./account/e2eAccountService");
  accountService = e2eAccount.e2eAccountService;
  Object.defineProperty(window, "__LUNA_E2E_ACCOUNT__", {
    value: e2eAccount.e2eAccountTestControl,
  });
}

if (import.meta.env.MODE === "live-canary") {
  Object.defineProperty(window, "__LUNA_LIVE_CANARY__", {
    value: {
      async synchronizeManagedAccess() {
        const session = await accountService.restoreSession();
        if (!session) return { ok: false, name: "SessionUnavailable", message: "No restored session.", status: null };
        try {
          await synchronizeManagedIntelligenceAccess(
            accountService,
            tauriConversationService,
            trustedDeviceService,
            session,
          );
          return { ok: true };
        } catch (error) {
          const failure = error as { name?: string; message?: string; context?: { status?: number } };
          return {
            ok: false,
            name: failure.name ?? "Error",
            message: failure.message ?? "Managed access synchronization failed.",
            status: failure.context?.status ?? null,
          };
        }
      },
    },
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
