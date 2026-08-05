"use client";

import { useEffect, useRef, type RefObject } from "react";

const FOCUSABLE = "button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])";

export function useModalFocus(open: boolean, root: RefObject<HTMLElement | null>, onClose: () => void) {
  const close = useRef(onClose);
  close.current = onClose;

  useEffect(() => {
    if (!open || !root.current) return;
    const returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const modal = root.current;
    const focusable = () => Array.from(modal.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((element) => !element.hasAttribute("hidden"));
    focusable()[0]?.focus();

    const handleKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        close.current();
        return;
      }
      if (event.key !== "Tab") return;
      const elements = focusable();
      if (elements.length === 0) return;
      const first = elements[0];
      const last = elements[elements.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };

    modal.addEventListener("keydown", handleKey);
    return () => {
      modal.removeEventListener("keydown", handleKey);
      returnFocus?.focus();
    };
  }, [open, root]);
}
