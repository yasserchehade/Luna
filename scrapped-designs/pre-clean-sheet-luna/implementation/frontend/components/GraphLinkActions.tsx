"use client";

import { useRouter } from "next/navigation";
import { useMemo, useState } from "react";

type HouseholdGraphNode = {
  id: string;
  node_type: string;
  display_name: string;
};

type EntityRelationship = {
  id: string;
  source_entity_type: string;
  source_entity_id: string;
  relationship_type: string;
  target_entity_type: string;
  target_entity_id: string;
};

type GraphLinkActionsProps = {
  nodes: HouseholdGraphNode[];
  relationships: EntityRelationship[];
  sourceId: string;
  sourceType: "bill" | "document";
};

type ActionState = "idle" | "working" | "error" | "success";

const RELATIONSHIP_EXAMPLES = [
  "related_to",
  "belongs_to",
  "issued_by",
  "supplies",
  "insures",
  "concerns",
];

function apiBaseUrl() {
  return process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8000";
}

function normalizedLabel(value: string) {
  return value.replaceAll("_", " ");
}

function nodeOptionLabel(node: HouseholdGraphNode) {
  return `${node.display_name} (${normalizedLabel(node.node_type)})`;
}

export function GraphLinkActions({
  nodes,
  relationships,
  sourceId,
  sourceType,
}: GraphLinkActionsProps) {
  const router = useRouter();
  const [state, setState] = useState<ActionState>("idle");
  const [relationshipType, setRelationshipType] = useState("related_to");
  const [message, setMessage] = useState<string | null>(null);

  const entityNodes = useMemo(
    () => nodes.filter((node) => node.node_type !== "document" && node.node_type !== "bill"),
    [nodes],
  );
  const nodesById = useMemo(
    () => new Map(nodes.map((node) => [node.id, node])),
    [nodes],
  );
  const currentRelationships = useMemo(
    () =>
      relationships.filter(
        (relationship) =>
          (relationship.source_entity_type === sourceType &&
            relationship.source_entity_id === sourceId) ||
          (relationship.target_entity_type === sourceType &&
            relationship.target_entity_id === sourceId),
      ),
    [relationships, sourceId, sourceType],
  );

  async function createLink(formData: FormData) {
    const targetEntityId = String(formData.get("target_entity_id") ?? "");
    if (!targetEntityId) {
      setState("error");
      setMessage("Choose an entity to link.");
      return;
    }

    setState("working");
    setMessage(null);

    try {
      const response = await fetch(`${apiBaseUrl()}/api/household/relationships`, {
        body: JSON.stringify({
          provenance_document_id: sourceType === "document" ? sourceId : null,
          relationship_type: String(formData.get("relationship_type") ?? "related_to"),
          source_entity_id: sourceId,
          source_entity_type: sourceType,
          target_entity_id: targetEntityId,
          target_entity_type: "entity",
        }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });

      if (!response.ok) {
        let detail = "Could not create graph link.";
        try {
          const body = (await response.json()) as { detail?: string };
          detail = body.detail ?? detail;
        } catch {
          detail = response.statusText || detail;
        }
        throw new Error(detail);
      }

      setState("success");
      setMessage("Linked.");
      router.refresh();
    } catch (error) {
      setState("error");
      setMessage(error instanceof Error ? error.message : "Could not create graph link.");
    }
  }

  return (
    <div className="graphLinkActions">
      <div className="relationshipMiniList">
        {currentRelationships.length === 0 ? (
          <span>No graph links</span>
        ) : (
          currentRelationships.map((relationship) => {
            const source = nodesById.get(relationship.source_entity_id);
            const target = nodesById.get(relationship.target_entity_id);
            return (
              <span key={relationship.id}>
                {source?.display_name ?? normalizedLabel(relationship.source_entity_type)}{" "}
                {normalizedLabel(relationship.relationship_type)}{" "}
                {target?.display_name ?? normalizedLabel(relationship.target_entity_type)}
              </span>
            );
          })
        )}
      </div>

      <form action={createLink} className="graphLinkForm">
        <select disabled={entityNodes.length === 0 || state === "working"} name="target_entity_id">
          <option value="">Select entity</option>
          {entityNodes.map((node) => (
            <option key={node.id} value={node.id}>
              {nodeOptionLabel(node)}
            </option>
          ))}
        </select>
        <input
          list={`relationship-examples-${sourceType}-${sourceId}`}
          name="relationship_type"
          onChange={(event) => setRelationshipType(event.target.value)}
          type="text"
          value={relationshipType}
        />
        <datalist id={`relationship-examples-${sourceType}-${sourceId}`}>
          {RELATIONSHIP_EXAMPLES.map((example) => (
            <option key={example} value={example} />
          ))}
        </datalist>
        <button disabled={entityNodes.length === 0 || state === "working"} type="submit">
          Link
        </button>
      </form>

      {message ? (
        <span className={state === "error" ? "miniError" : "miniSuccess"}>{message}</span>
      ) : null}
    </div>
  );
}
