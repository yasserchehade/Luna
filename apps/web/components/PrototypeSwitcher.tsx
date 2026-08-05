"use client";

import { useEffect } from "react";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import type { PrototypeVariantKey } from "../lib/prototypeState";

const variants: Array<{ key: PrototypeVariantKey; name: string }> = [
  { key: "A", name: "Briefing stream" },
  { key: "B", name: "Luna's desk" },
  { key: "C", name: "Conversation first" },
];

export function PrototypeSwitcher({ current }: { current: PrototypeVariantKey }) {
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();

  const cycle = (direction: -1 | 1) => {
    const currentIndex = Math.max(0, variants.findIndex((variant) => variant.key === current));
    const next = variants[(currentIndex + direction + variants.length) % variants.length];
    const params = new URLSearchParams(searchParams.toString());
    params.set("variant", next.key);
    router.replace(`${pathname}?${params.toString()}`, { scroll: false });
  };

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.matches("input, textarea, [contenteditable='true']") || target?.closest("[contenteditable='true']")) return;
      if (event.key === "ArrowLeft") cycle(-1);
      if (event.key === "ArrowRight") cycle(1);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  if (process.env.NODE_ENV === "production") return null;
  const active = variants.find((variant) => variant.key === current) ?? variants[0];

  return (
    <div className="prototype-switcher" aria-label="Prototype variants">
      <button type="button" onClick={() => cycle(-1)} aria-label="Previous variant">←</button>
      <div><span>Web-first prototype</span><strong>{active.key} — {active.name}</strong></div>
      <button type="button" onClick={() => cycle(1)} aria-label="Next variant">→</button>
    </div>
  );
}
