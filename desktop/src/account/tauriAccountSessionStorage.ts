import { invoke } from "@tauri-apps/api/core";
import type { AccountSessionStorage } from "./accountService";

export const tauriAccountSessionStorage: AccountSessionStorage = {
  getItem(key) {
    return invoke<string | null>("get_account_session_item", { key });
  },
  async setItem(key, value) {
    await invoke("set_account_session_item", { key, value });
  },
  async removeItem(key) {
    await invoke("remove_account_session_item", { key });
  },
};
