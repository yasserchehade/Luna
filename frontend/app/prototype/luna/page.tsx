"use client";

// Three variants of Luna's primary desktop surface, switchable via ?variant=, on a throwaway prototype route.

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";
import { PrototypeSwitcher } from "../../../components/prototype/PrototypeSwitcher";
import styles from "./prototype.module.css";

const variants = [
  { key: "A", name: "Conversation desk" },
  { key: "B", name: "Document workbench" },
  { key: "C", name: "Quiet focus" },
];

const navItems = ["Luna", "To do", "Cabinet", "History", "Options"];

function LunaMark({ compact = false }: { compact?: boolean }) {
  return (
    <div className={styles.brand}>
      <span className={styles.mark}>L</span>
      {!compact && <strong>Luna</strong>}
    </div>
  );
}

function StatusPill({ children, tone = "green" }: { children: React.ReactNode; tone?: "green" | "amber" | "slate" }) {
  return <span className={`${styles.status} ${styles[tone]}`}>{children}</span>;
}

function DocumentSummary() {
  return (
    <div className={styles.documentSummary}>
      <div className={styles.documentHeader}>
        <div className={styles.fileIcon}>PDF</div>
        <div>
          <strong>Electricity bill — May 2026</strong>
          <span>Origin Energy · 3 pages · received today</span>
        </div>
        <button type="button" aria-label="More document actions">•••</button>
      </div>
      <div className={styles.fields}>
        <div><span>Addressee</span><strong>Yasser Chehade</strong><StatusPill>Confirmed</StatusPill></div>
        <div><span>Property</span><strong>18 Willow Street</strong><StatusPill tone="amber">Needs checking</StatusPill></div>
        <div><span>Amount due</span><strong>$286.40 · 28 May</strong><StatusPill>Confirmed</StatusPill></div>
      </div>
      <div className={styles.localNote}><span>●</span> Read locally on this device · nothing shared externally</div>
    </div>
  );
}

function Composer({ label = "Message Luna or attach a document" }: { label?: string }) {
  return (
    <div className={styles.composer}>
      <button type="button" aria-label="Attach document">＋</button>
      <span>{label}</span>
      <button type="button" className={styles.send} aria-label="Send message">↑</button>
    </div>
  );
}

function VariantA() {
  return (
    <main className={`${styles.prototype} ${styles.variantA}`}>
      <aside className={styles.aSidebar}>
        <LunaMark />
        <button className={styles.newConversation}>＋ <span>New conversation</span></button>
        <nav>
          {navItems.map((item, index) => <button className={index === 0 ? styles.active : ""} key={item}><span>{["✦", "✓", "▱", "↺", "⌘"][index]}</span>{item}{item === "To do" && <b>2</b>}</button>)}
        </nav>
        <div className={styles.recents}>
          <span>Recent</span>
          <button>Origin Energy bill</button>
          <button>Weekly household brief</button>
          <button>Lease renewal</button>
        </div>
        <div className={styles.member}><span>YC</span><div><strong>Yasser</strong><small>Household organiser</small></div><button>•••</button></div>
      </aside>

      <section className={styles.aConversation}>
        <header><div><span>Today</span><h1>Origin Energy bill</h1></div><StatusPill tone="slate">Private conversation</StatusPill></header>
        <div className={styles.aMessages}>
          <div className={styles.userMessage}><span className={styles.avatar}>YC</span><p>Please take care of this bill for 18 Willow Street.</p></div>
          <div className={styles.lunaMessage}><span className={styles.lunaAvatar}>L</span><div><p>I’ve read the bill locally. Most of it is clear, but this is the first document I’ve seen from <strong>Origin Energy</strong> for this property.</p><DocumentSummary /><div className={styles.question}><strong>Has Origin Energy replaced your previous electricity provider for 18 Willow Street?</strong><p>Your answer will help me choose the right cabinet destination. I’ll show you the filing decision before I move anything.</p><div><button className={styles.primary}>Yes, they’re the new provider</button><button>No, this needs checking</button></div></div></div></div>
        </div>
        <Composer />
      </section>

      <aside className={styles.aContext}>
        <header><span>Household context</span><button>×</button></header>
        <div className={styles.contextBlock}><span>Property</span><strong>18 Willow Street</strong><small>Primary residence · added 2024</small></div>
        <div className={styles.contextBlock}><span>Current service</span><strong>Electricity</strong><small>AGL · last bill 26 April 2026</small></div>
        <div className={styles.contextBlock}><span>Proposed destination</span><strong>Household / Properties / 18 Willow Street / Utilities / Electricity</strong><small>Original filename will be preserved</small></div>
        <div className={styles.rulePreview}><span>After you answer</span><p>Luna can remember this provider for future electricity bills at this property.</p><label><input type="checkbox" /> File matching bills without asking</label></div>
      </aside>
    </main>
  );
}

function VariantB() {
  return (
    <main className={`${styles.prototype} ${styles.variantB}`}>
      <header className={styles.bTopbar}>
        <LunaMark />
        <nav>{navItems.map((item, index) => <button className={index === 0 ? styles.active : ""} key={item}>{item}{item === "To do" && <b>2</b>}</button>)}</nav>
        <div className={styles.device}><span>●</span> Processing locally</div>
        <div className={styles.memberMini}>YC</div>
      </header>

      <section className={styles.bTitle}>
        <div><span>DOCUMENT HANDLING · TODAY</span><h1>One document needs your direction</h1><p>Luna has paused before changing the household’s known electricity provider.</p></div>
        <button>View all to-do items <span>→</span></button>
      </section>

      <section className={styles.bWorkspace}>
        <aside className={styles.queue}>
          <header><strong>To do</strong><span>2 open</span></header>
          <button className={styles.selectedQueue}><span className={styles.queueIcon}>⚡</span><div><strong>Confirm new provider</strong><small>Origin Energy · 18 Willow St</small><em>Now</em></div></button>
          <button><span className={styles.queueIcon}>⌂</span><div><strong>Identify new address</strong><small>Council rates notice</small><em>Yesterday</em></div></button>
          <div className={styles.completed}><span>Completed today</span><strong>3</strong></div>
        </aside>

        <article className={styles.paperArea}>
          <div className={styles.paperToolbar}><span>Origin_Energy_May_2026.pdf</span><div><button>−</button><span>92%</span><button>＋</button></div></div>
          <div className={styles.paper}>
            <div className={styles.fakeLogo}>origin</div>
            <span className={styles.billLabel}>Electricity bill</span>
            <h2>$286.40</h2><p>Due 28 May 2026</p>
            <div className={styles.billRule}></div>
            <div className={styles.billGrid}><div><span>Account holder</span><strong>Yasser Chehade</strong></div><div><span>Supply address</span><strong>18 Willow Street</strong></div></div>
            <div className={styles.usageBars}><i></i><i></i><i></i><i></i><i></i><i></i><i></i></div>
          </div>
          <div className={`${styles.highlight} ${styles.one}`}>Addressee confirmed</div>
          <div className={`${styles.highlight} ${styles.two}`}>Property needs checking</div>
        </article>

        <aside className={styles.inspector}>
          <div className={styles.inspectorHead}><span className={styles.lunaAvatar}>L</span><div><strong>Luna’s review</strong><small>Ready for your direction</small></div></div>
          <p>I found a provider change that affects the household context.</p>
          <div className={styles.changeCompare}><div><span>Previously</span><strong>AGL</strong><small>Last seen 26 Apr</small></div><span>→</span><div><span>This bill</span><strong>Origin Energy</strong><small>First time seen</small></div></div>
          <h3>Did Origin Energy replace AGL for this property?</h3>
          <button className={styles.fullPrimary}>Yes — update the provider</button>
          <button className={styles.fullSecondary}>No — help me correct this</button>
          <div className={styles.nextStep}><span>Next</span><p>I’ll propose a cabinet destination and ask whether to remember this decision.</p></div>
        </aside>
      </section>
    </main>
  );
}

function VariantC() {
  return (
    <main className={`${styles.prototype} ${styles.variantC}`}>
      <header className={styles.cHeader}>
        <LunaMark />
        <nav>{navItems.map((item, index) => <button className={index === 0 ? styles.active : ""} key={item}>{item}{item === "To do" && <b>2</b>}</button>)}</nav>
        <button className={styles.search}>⌕ <span>Search Luna</span><kbd>⌘ K</kbd></button>
        <div className={styles.memberMini}>YC</div>
      </header>
      <section className={styles.cCanvas}>
        <div className={styles.cMeta}><span>Saturday, 11 July</span><StatusPill tone="slate">Private</StatusPill></div>
        <div className={styles.cIntro}><span className={styles.cOrb}>L</span><div><h1>I need one detail before I file this.</h1><p>This is the first Origin Energy bill I’ve seen for 18 Willow Street. Everything else looks consistent with your household records.</p></div></div>
        <div className={styles.cEvidence}>
          <div className={styles.cBill}><div className={styles.fileIcon}>PDF</div><div><strong>Electricity bill — May 2026</strong><span>Origin Energy · $286.40 due 28 May</span></div><button>Preview</button></div>
          <div className={styles.cFacts}><div><span>Understood</span><strong>Electricity bill</strong><small>Yasser · 18 Willow Street</small></div><div><span>Changed</span><strong>Service provider</strong><small>AGL → Origin Energy</small></div><div><span>Proposed filing</span><strong>Utilities / Electricity</strong><small>Original remains untouched</small></div></div>
        </div>
        <div className={styles.cPrompt}>
          <h2>Has Origin Energy replaced AGL as the electricity provider?</h2>
          <div><button className={styles.primary}>Yes, update the household context</button><button>No, let me explain</button></div>
          <p>I won’t move the document until you answer.</p>
        </div>
        <Composer label="Tell Luna what changed…" />
        <div className={styles.cPrivacy}><span>◉</span><div><strong>Local inspection complete</strong><small>The document has not left this device.</small></div><button>How Luna handled this</button></div>
      </section>
    </main>
  );
}

function LunaPrototypeContent() {
  const searchParams = useSearchParams();
  const requested = searchParams.get("variant")?.toUpperCase() ?? "A";
  const current = variants.some((variant) => variant.key === requested) ? requested : "A";

  return (
    <>
      {current === "A" && <VariantA />}
      {current === "B" && <VariantB />}
      {current === "C" && <VariantC />}
      <PrototypeSwitcher variants={variants} current={current} />
    </>
  );
}

export default function LunaPrototypePage() {
  return (
    <Suspense fallback={<VariantA />}>
      <LunaPrototypeContent />
    </Suspense>
  );
}
