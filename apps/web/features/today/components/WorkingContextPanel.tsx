"use client";

import { useEffect, useRef, useState } from "react";
import { AppIcon } from "../../../components/AppIcon";
import type { FactCorrectionInput, HouseholdWorkView } from "../contracts";
import { useModalFocus } from "./useModalFocus";

export function WorkingContextPanel({
  work,
  drawer = false,
  correctionOpen,
  pending,
  onClose,
  onOpenCorrection,
  onCancelCorrection,
  onCorrect,
}: {
  work: HouseholdWorkView | null;
  drawer?: boolean;
  correctionOpen: boolean;
  pending: boolean;
  onClose: () => void;
  onOpenCorrection: () => void;
  onCancelCorrection: () => void;
  onCorrect: (input: FactCorrectionInput) => void;
}) {
  const panel = useRef<HTMLElement>(null);
  const [factKey, setFactKey] = useState(work?.facts[0]?.key ?? "");
  const selectedFact = work?.facts.find((fact) => fact.key === factKey) ?? work?.facts[0];
  const [value, setValue] = useState(selectedFact?.value ?? "");

  useEffect(() => {
    const firstFact = work?.facts[0];
    setFactKey(firstFact?.key ?? "");
    setValue(firstFact?.value ?? "");
  }, [work?.id]);

  useModalFocus(drawer, panel, onClose);

  const chooseFact = (nextKey: string) => {
    setFactKey(nextKey);
    setValue(work?.facts.find((fact) => fact.key === nextKey)?.value ?? "");
  };

  return (
    <aside ref={panel} className={`today-context-panel ${drawer ? "drawer" : ""}`} aria-label="Working context" role={drawer ? "dialog" : undefined} aria-modal={drawer || undefined}>
      <header>
        <div><span>Working context</span><strong>{work?.title ?? "Nothing selected"}</strong></div>
        {drawer && <button type="button" aria-label="Close work details" onClick={onClose}><AppIcon name="close" /></button>}
      </header>
      {work ? (
        <div className="today-context-content">
          <section><span className="context-label">Currently working on</span><p>{work.activity}</p></section>
          <section><span className="context-label">Relevant source</span><div className="context-source"><AppIcon name="source" /><div><strong>{work.source.label}</strong><small>{work.source.detail}</small></div></div></section>
          <section><span className="context-label">Household</span><div className="entity-pill"><AppIcon name="home" />{work.householdEntity}</div></section>
          <section>
            <span className="context-label">What I understand</span>
            <dl>{work.facts.map((fact) => <div key={fact.key}><dt>{fact.label}</dt><dd>{fact.value}</dd></div>)}</dl>
          </section>
          <section><span className="context-label">What I still need</span><p>{work.needs ?? "Nothing from you right now."}</p></section>
          <section><span className="context-label">Recommended action</span><p className="context-recommendation">{work.recommendation}</p></section>

          {correctionOpen ? (
            <form className="context-correction" onSubmit={(event) => { event.preventDefault(); if (work && factKey && value.trim()) onCorrect({ workId: work.id, factKey, value }); }}>
              <label htmlFor="fact-to-correct">Fact to correct</label>
              <select id="fact-to-correct" value={factKey} onChange={(event) => chooseFact(event.target.value)}>
                {work.facts.map((fact) => <option value={fact.key} key={fact.key}>{fact.label}</option>)}
              </select>
              <label htmlFor="corrected-value">Correct value</label>
              <input id="corrected-value" value={value} onChange={(event) => setValue(event.target.value)} />
              <div><button className="primary-action" type="submit" disabled={pending}>{pending ? "Saving…" : "Save correction"}</button><button type="button" onClick={onCancelCorrection}>Cancel</button></div>
            </form>
          ) : <button className="context-secondary-button" type="button" onClick={onOpenCorrection}><AppIcon name="details" /> Correct a fact</button>}

          <details className="work-details">
            <summary>Details</summary>
            <p>Source: {work.source.detail}</p>
          </details>
        </div>
      ) : <div className="today-context-empty"><AppIcon name="details" /><p>Select household work to see the context Luna is using.</p></div>}
    </aside>
  );
}
