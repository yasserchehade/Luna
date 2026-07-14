import { invoke } from "@tauri-apps/api/core";

export type CabinetStorage = "cloudSynchronized" | "local";

export type CabinetPreview = {
  root: string;
  sections: string[];
};

export type CabinetConfiguration = CabinetPreview & {
  storage: CabinetStorage;
};

export type CabinetValidation = {
  configuration: CabinetConfiguration;
  availability: "ready" | "unavailable";
};

export interface CabinetService {
  selectFolder(): Promise<string | null>;
  preview(root: string, sections: string[]): Promise<CabinetPreview>;
  create(householdId: string, storage: CabinetStorage, preview: CabinetPreview): Promise<CabinetConfiguration>;
  validate(householdId: string): Promise<CabinetValidation | null>;
}

export const recommendedCabinetSections = [
  "Bills & Services",
  "Identity",
  "Legal",
  "Property",
  "Purchases & Warranties",
] as const;

export const tauriCabinetService: CabinetService = {
  selectFolder() {
    return invoke<string | null>("select_cabinet_folder");
  },
  preview(root, sections) {
    return invoke<CabinetPreview>("preview_cabinet", { root, sections });
  },
  create(householdId, storage, preview) {
    return invoke<CabinetConfiguration>("create_cabinet", { householdId, storage, preview });
  },
  validate(householdId) {
    return invoke<CabinetValidation | null>("validate_cabinet", { householdId });
  },
};
