import { useState } from "react";
import type { AccountService, HouseholdSession } from "../account/accountService";
import { FilingRulesOptions } from "../conversation/FilingRulesOptions";
import type { ConversationService } from "../conversation/conversationService";
import { CloudAssistanceOptions } from "./CloudAssistanceOptions";
import { TrustedDevicesOptions } from "../trusted-device/TrustedDevicesOptions";
import type { TrustedDeviceService } from "../trusted-device/trustedDeviceService";

type OptionsSection = "devices" | "rules" | "cloud";

export function OptionsWorkspace({
  accountService,
  conversationService,
  onSignOut,
  session,
  trustedDeviceService,
}: {
  accountService: AccountService;
  conversationService: ConversationService;
  onSignOut: () => void | Promise<void>;
  session: HouseholdSession;
  trustedDeviceService: TrustedDeviceService;
}) {
  const [section, setSection] = useState<OptionsSection>("devices");

  return <main className="conversation options-view options-workspace">
    <header><div><small>Household settings</small><h1>Options</h1></div><span>Choose a section</span></header>
    <nav className="options-section-nav" aria-label="Options sections">
      <button
        aria-current={section === "devices" ? "page" : undefined}
        aria-label="Trusted devices options"
        className={section === "devices" ? "selected" : undefined}
        onClick={() => setSection("devices")}
        type="button"
      >Trusted devices</button>
      <button
        aria-current={section === "rules" ? "page" : undefined}
        aria-label="Learned Filing Rules options"
        className={section === "rules" ? "selected" : undefined}
        onClick={() => setSection("rules")}
        type="button"
      >Learned Filing Rules</button>
      <button
        aria-current={section === "cloud" ? "page" : undefined}
        aria-label="Cloud assistance options"
        className={section === "cloud" ? "selected" : undefined}
        onClick={() => setSection("cloud")}
        type="button"
      >Cloud assistance</button>
    </nav>
    {section === "devices"
      ? <TrustedDevicesOptions
        accountService={accountService}
        onSignOut={onSignOut}
        session={session}
        trustedDeviceService={trustedDeviceService}
      />
      : section === "rules" ? <FilingRulesOptions
        conversationService={conversationService}
        householdId={session.householdId}
      /> : <CloudAssistanceOptions conversationService={conversationService} householdId={session.householdId} />}
  </main>;
}
