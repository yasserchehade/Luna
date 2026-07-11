"use client";

import { useState } from "react";
import { UploadBillForm } from "./UploadBillForm";

export function CreateMenu() {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <div className="createMenu">
      <button
        aria-expanded={isOpen}
        aria-haspopup="menu"
        onClick={() => setIsOpen((current) => !current)}
        type="button"
      >
        + Add something
      </button>
      {isOpen ? (
        <div className="createMenuPanel" role="menu">
          <div className="createMenuSection">
            <strong>Upload household record</strong>
            <UploadBillForm compact />
          </div>
          <a href="/?tab=structure&mode=add" role="menuitem">
            Add household item
          </a>
          <a href="/?tab=structure&mode=link" role="menuitem">
            Link items
          </a>
          <a href="/?tab=assistant" role="menuitem">
            Ask Luna
          </a>
        </div>
      ) : null}
    </div>
  );
}
