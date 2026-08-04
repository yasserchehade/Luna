import type { SVGProps } from "react";

export type IconName =
  | "today" | "conversation" | "calendar" | "cabinet" | "household" | "history" | "settings"
  | "paperclip" | "send" | "check" | "clock" | "source" | "home" | "spark" | "menu"
  | "close" | "details" | "alert";

const paths: Record<IconName, React.ReactNode> = {
  today: <><path d="M5 4.5h14v15H5z"/><path d="M8 2.5v4M16 2.5v4M5 8.5h14"/></>,
  conversation: <path d="M4 5.5h16v11H9l-5 4v-15Z"/>,
  calendar: <><path d="M4 5h16v15H4z"/><path d="M8 2v6M16 2v6M4 9h16"/></>,
  cabinet: <><path d="M4 4h16v6H4zM4 10h16v10H4z"/><path d="M10 7h4M10 14h4"/></>,
  household: <><path d="m3 11 9-7 9 7"/><path d="M6 10v10h12V10M10 20v-6h4v6"/></>,
  history: <><path d="M4 12a8 8 0 1 0 2-5.3L4 9"/><path d="M4 4v5h5M12 8v5l3 2"/></>,
  settings: <><circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9 7 7M17 17l2.1 2.1M19.1 4.9 17 7M7 17l-2.1 2.1"/></>,
  paperclip: <path d="m9 17 7.5-7.5a3 3 0 0 0-4.2-4.2L5.5 12a5 5 0 0 0 7.1 7.1l7-7"/>,
  send: <><path d="m3 11 18-8-8 18-2-8-8-2Z"/><path d="m11 13 4-4"/></>,
  check: <path d="m5 12 4 4L19 6"/>,
  clock: <><circle cx="12" cy="12" r="9"/><path d="M12 7v6l4 2"/></>,
  source: <><path d="M7 3h7l4 4v14H7z"/><path d="M14 3v5h5M10 13h5M10 17h5"/></>,
  home: <><path d="m3 11 9-7 9 7"/><path d="M6 10v10h12V10"/></>,
  spark: <path d="m12 2 1.6 5.4L19 9l-5.4 1.6L12 16l-1.6-5.4L5 9l5.4-1.6L12 2ZM19 16l.8 2.2L22 19l-2.2.8L19 22l-.8-2.2L16 19l2.2-.8L19 16Z"/>,
  menu: <path d="M4 7h16M4 12h16M4 17h16"/>,
  close: <path d="m6 6 12 12M18 6 6 18"/>,
  details: <><circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7h.01"/></>,
  alert: <><path d="m12 3 10 18H2L12 3Z"/><path d="M12 9v5M12 18h.01"/></>,
};

export function AppIcon({ name, ...props }: { name: IconName } & SVGProps<SVGSVGElement>) {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" {...props}>
      {paths[name]}
    </svg>
  );
}
