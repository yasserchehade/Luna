"use client";

import { Suspense } from "react";
import { useSearchParams } from "next/navigation";
import { WebFirstPrototype } from "../../../components/WebFirstPrototype";
import type { PrototypeFixtureState, PrototypeVariantKey } from "../../../lib/prototypeState";

const variants: PrototypeVariantKey[] = ["A", "B", "C"];
const fixtureStates: PrototypeFixtureState[] = ["ready", "loading", "empty", "error"];

function WebFirstPrototypeContent() {
  const searchParams = useSearchParams();
  const requestedVariant = searchParams.get("variant") as PrototypeVariantKey | null;
  const requestedState = searchParams.get("state") as PrototypeFixtureState | null;
  const variant = requestedVariant && variants.includes(requestedVariant) ? requestedVariant : "A";
  const fixtureState = requestedState && fixtureStates.includes(requestedState) ? requestedState : "ready";

  return <WebFirstPrototype variant={variant} fixtureState={fixtureState} />;
}

export default function WebFirstPrototypePage() {
  return <Suspense fallback={null}><WebFirstPrototypeContent /></Suspense>;
}
