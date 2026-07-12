import { FormEvent, useState } from "react";
import "./App.css";

const destinations = ["Luna", "To do", "Cabinet", "History", "Options"] as const;

export default function App() {
  const [draft, setDraft] = useState("");
  const [messages, setMessages] = useState<string[]>([]);

  const submitMessage = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const message = draft.trim();
    if (!message) return;
    setMessages((current) => [...current, message]);
    setDraft("");
  };

  return (
    <div className="luna-shell">
      <aside className="sidebar">
        <div className="brand"><span aria-hidden="true">L</span><strong>Luna</strong></div>
        <button className="new-conversation" type="button">＋ New conversation</button>
        <nav aria-label="Primary destinations">
          {destinations.map((destination) => (
            <button
              aria-current={destination === "Luna" ? "page" : undefined}
              aria-label={destination}
              className={destination === "Luna" ? "active" : undefined}
              key={destination}
              type="button"
            >
              <span>{destination}</span>
              {destination === "To do" && <small aria-hidden="true">2</small>}
            </button>
          ))}
        </nav>
        <div className="member"><span>YC</span><div><strong>Yasser</strong><small>Household organiser</small></div></div>
      </aside>

      <main className="conversation">
        <header><div><small>Today</small><h1>New conversation</h1></div><span>Private conversation</span></header>
        <section className="messages" aria-label="Conversation">
          <article className="luna-message"><span aria-hidden="true">L</span><p>What would you like me to take care of?</p></article>
          {messages.map((message, index) => (
            <article className="member-message" key={`${message}-${index}`}><span aria-hidden="true">YC</span><p>{message}</p></article>
          ))}
        </section>
        <form className="composer" onSubmit={submitMessage}>
          <label htmlFor="message-composer">Message Luna</label>
          <textarea
            id="message-composer"
            onChange={(event) => setDraft(event.target.value)}
            placeholder="Message Luna or attach a document"
            rows={1}
            value={draft}
          />
          <button type="submit" aria-label="Send message">↑</button>
        </form>
      </main>

      <aside className="context-panel">
        <header>Household context</header>
        <div><small>Desk status</small><strong>Ready</strong><p>Your cabinet will appear here after onboarding.</p></div>
        <div className="privacy"><small>Processing</small><strong>Local by default</strong><p>Luna will ask before using Cloud Assistance.</p></div>
      </aside>
    </div>
  );
}
