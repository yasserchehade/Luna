"use client";

import { useRef } from "react";
import { AppIcon, type IconName } from "../../../components/AppIcon";
import type { TodayNavigationKey } from "../contracts";
import { useModalFocus } from "./useModalFocus";

export const navigationItems: Array<{ label: TodayNavigationKey; icon: IconName }> = [
  { label: "Today", icon: "today" },
  { label: "Conversations", icon: "conversation" },
  { label: "Calendar", icon: "calendar" },
  { label: "Cabinet", icon: "cabinet" },
  { label: "Household", icon: "household" },
  { label: "History", icon: "history" },
  { label: "Settings", icon: "settings" },
];

function LunaMark() {
  return <span className="luna-mark" aria-hidden="true"><AppIcon name="spark" /></span>;
}

export function PrimaryNavigation({
  active,
  household,
  initials,
  onNavigate,
}: {
  active: TodayNavigationKey;
  household: string;
  initials: string;
  onNavigate: (destination: TodayNavigationKey) => void;
}) {
  return (
    <aside className="today-sidebar">
      <div className="today-brand"><LunaMark /><strong>Luna</strong></div>
      <nav aria-label="Primary navigation">
        {navigationItems.map((item) => (
          <button
            key={item.label}
            type="button"
            className={active === item.label ? "active" : ""}
            onClick={() => onNavigate(item.label)}
            aria-label={item.label}
            aria-current={active === item.label ? "page" : undefined}
          >
            <AppIcon name={item.icon} />
            <span>{item.label}</span>
          </button>
        ))}
      </nav>
      <div className="today-household">
        <span>{initials}</span>
        <div><strong>{household}</strong><small>All caught up</small></div>
      </div>
    </aside>
  );
}

export function MobileNavigation({ active, onNavigate }: {
  active: TodayNavigationKey;
  onNavigate: (destination: TodayNavigationKey) => void;
}) {
  return (
    <nav className="today-mobile-navigation" aria-label="Mobile navigation">
      {navigationItems.slice(0, 5).map((item) => (
        <button
          key={item.label}
          type="button"
          className={active === item.label ? "active" : ""}
          onClick={() => onNavigate(item.label)}
          aria-label={item.label}
          aria-current={active === item.label ? "page" : undefined}
        >
          <AppIcon name={item.icon} />
          <span>{item.label}</span>
        </button>
      ))}
    </nav>
  );
}

export function MobileMenu({ active, open, onClose, onNavigate }: {
  active: TodayNavigationKey;
  open: boolean;
  onClose: () => void;
  onNavigate: (destination: TodayNavigationKey) => void;
}) {
  const menu = useRef<HTMLElement>(null);
  useModalFocus(open, menu, onClose);
  if (!open) return null;
  return (
    <div className="today-drawer-backdrop" onMouseDown={onClose}>
      <section ref={menu} className="today-mobile-menu" role="dialog" aria-modal="true" aria-label="All navigation" onMouseDown={(event) => event.stopPropagation()}>
        <header><strong>Navigate</strong><button type="button" aria-label="Close navigation" onClick={onClose}><AppIcon name="close" /></button></header>
        <nav aria-label="All destinations">
          {navigationItems.map((item) => (
            <button key={item.label} type="button" aria-current={active === item.label ? "page" : undefined} onClick={() => { onNavigate(item.label); onClose(); }}>
              <AppIcon name={item.icon} />{item.label}
            </button>
          ))}
        </nav>
      </section>
    </div>
  );
}

export { LunaMark };
