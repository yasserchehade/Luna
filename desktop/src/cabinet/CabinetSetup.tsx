import { useState } from "react";
import type { HouseholdSession } from "../account/accountService";
import {
  type CabinetConfiguration,
  type CabinetPreview,
  type CabinetService,
  recommendedCabinetSections,
} from "./cabinetService";

type CabinetSetupProps = {
  cabinetService: CabinetService;
  session: HouseholdSession;
  unavailableRoot?: string;
  onConfigured: (configuration: CabinetConfiguration) => void;
};

export function CabinetSetup({ cabinetService, session, unavailableRoot, onConfigured }: CabinetSetupProps) {
  const [storageGuidance, setStorageGuidance] = useState<"cloud" | "direct">("cloud");
  const [preview, setPreview] = useState<CabinetPreview | null>(null);
  const [sections, setSections] = useState<string[]>([...recommendedCabinetSections]);
  const [newSection, setNewSection] = useState("");
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  const chooseFolder = async () => {
    setError("");
    setIsSubmitting(true);
    try {
      const selected = await cabinetService.selectFolder();
      if (selected) setPreview(await cabinetService.preview(selected, sections));
    } catch {
      setError("Luna could not use that folder. Choose a writable folder and try again.");
    } finally {
      setIsSubmitting(false);
    }
  };

  const createCabinet = async () => {
    if (!preview) return;
    setError("");
    setIsSubmitting(true);
    try {
      onConfigured(await cabinetService.create(session.householdId, {
        root: preview.root,
        sections,
      }));
    } catch {
      setError("Luna could not create the cabinet. Nothing was saved; check the folder and section names, then try again.");
    } finally {
      setIsSubmitting(false);
    }
  };

  if (!preview) {
    return <main className="account-screen"><section className="account-card cabinet-setup-card">
      <div className="account-brand"><span aria-hidden="true">L</span><strong>Luna</strong></div>
      <p className="eyebrow">Desk setup</p>
      <h1>Give Luna a desk</h1>
      <p>Choose where the {session.householdName} cabinet will live. The cabinet remains an ordinary folder you own and can use without Luna.</p>
      {unavailableRoot && <p role="status" className="session-notice">The remembered cabinet at <strong>{unavailableRoot}</strong> is unavailable. Luna will not redirect it without your direction.</p>}
      <div className="storage-options" role="group" aria-label="Cabinet storage">
        <button aria-label="Cloud-synchronised storage" aria-pressed={storageGuidance === "cloud"} className={storageGuidance === "cloud" ? "selected" : undefined} onClick={() => setStorageGuidance("cloud")} type="button">
          <strong>Cloud-synchronised storage</strong>
          <span>Recommended · choose a folder inside OneDrive, iCloud Drive, Dropbox or another service you control.</span>
        </button>
        <button aria-label="Local or network storage" aria-pressed={storageGuidance === "direct"} className={storageGuidance === "direct" ? "selected" : undefined} onClick={() => setStorageGuidance("direct")} type="button">
          <strong>Local or network folder</strong>
          <span>Keep the cabinet on this computer, an external drive or a network location you control.</span>
        </button>
      </div>
      {error && <p role="alert" className="account-error">{error}</p>}
      <button type="button" disabled={isSubmitting} onClick={chooseFolder}>Choose cabinet folder</button>
    </section></main>;
  }

  return <main className="account-screen"><section className="account-card cabinet-setup-card">
    <div className="account-brand"><span aria-hidden="true">L</span><strong>Luna</strong></div>
    <p className="eyebrow">Cabinet preview</p>
    <h1>Review your cabinet</h1>
    <p>No folders have been created yet. Rename, remove or add sections before confirming.</p>
    <p className="selected-folder"><small>Selected folder</small><strong>{preview.root}</strong></p>
    <div className="cabinet-section-editor">
      {sections.map((section, index) => <div className="cabinet-section-row" key={index}>
        <input aria-label={`Cabinet section ${index + 1}`} name="cabinet-section" onChange={(event) => setSections((current) => current.map((value, position) => position === index ? event.target.value : value))} value={section} />
        <button aria-label={`Remove ${section || `section ${index + 1}`}`} disabled={sections.length === 1} onClick={() => setSections((current) => current.filter((_, position) => position !== index))} type="button">Remove</button>
      </div>)}
      <div className="cabinet-section-row">
        <input aria-label="New cabinet section" onChange={(event) => setNewSection(event.target.value)} placeholder="Add another section" value={newSection} />
        <button disabled={!newSection.trim()} onClick={() => { setSections((current) => [...current, newSection.trim()]); setNewSection(""); }} type="button">Add section</button>
      </div>
    </div>
    {error && <p role="alert" className="account-error">{error}</p>}
    <button type="button" disabled={isSubmitting} onClick={createCabinet}>Create cabinet</button>
    <div className="account-switch"><button type="button" disabled={isSubmitting} onClick={() => setPreview(null)}>Choose a different folder</button></div>
  </section></main>;
}
