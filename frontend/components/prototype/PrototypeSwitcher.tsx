"use client";

import { useEffect } from "react";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import styles from "./PrototypeSwitcher.module.css";

export type PrototypeVariant = {
  key: string;
  name: string;
};

export function PrototypeSwitcher({
  variants,
  current,
}: {
  variants: PrototypeVariant[];
  current: string;
}) {
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();

  const cycle = (direction: -1 | 1) => {
    const currentIndex = Math.max(
      0,
      variants.findIndex((variant) => variant.key === current),
    );
    const nextIndex = (currentIndex + direction + variants.length) % variants.length;
    const params = new URLSearchParams(searchParams.toString());
    params.set("variant", variants[nextIndex].key);
    router.replace(`${pathname}?${params.toString()}`, { scroll: false });
  };

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (
        target?.matches("input, textarea, [contenteditable='true']") ||
        (target && target.closest("[contenteditable='true']"))
      ) {
        return;
      }
      if (event.key === "ArrowLeft") cycle(-1);
      if (event.key === "ArrowRight") cycle(1);
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  });

  if (process.env.NODE_ENV === "production") return null;

  const active = variants.find((variant) => variant.key === current) ?? variants[0];

  return (
    <div className={styles.switcher} aria-label="Prototype variants">
      <button type="button" onClick={() => cycle(-1)} aria-label="Previous variant">
        ←
      </button>
      <div>
        <span>UI prototype</span>
        <strong>{active.key} — {active.name}</strong>
      </div>
      <button type="button" onClick={() => cycle(1)} aria-label="Next variant">
        →
      </button>
    </div>
  );
}
