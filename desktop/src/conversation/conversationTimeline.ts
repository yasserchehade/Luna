export type TimelineMessage = {
  id: number;
  author: string;
  linkedDocumentArrival: number | null;
};

export type TimelineArrival = {
  id: number;
};

export type ConversationTimelineEntry<
  Message extends TimelineMessage,
  Arrival extends TimelineArrival,
> =
  | { kind: "message"; message: Message }
  | { kind: "arrival"; arrival: Arrival };

export function buildConversationTimeline<
  Message extends TimelineMessage,
  Arrival extends TimelineArrival,
>(messages: Message[], arrivals: Arrival[]): ConversationTimelineEntry<Message, Arrival>[] {
  const arrivalById = new Map(arrivals.map((arrival) => [arrival.id, arrival]));
  const anchoredArrivalIds = new Set(
    messages
      .filter((message) => message.author === "attachment")
      .map((message) => message.linkedDocumentArrival)
      .filter((arrivalId): arrivalId is number => arrivalId !== null && arrivalById.has(arrivalId)),
  );
  const legacyArrivals = [...arrivals]
    .filter((arrival) => !anchoredArrivalIds.has(arrival.id))
    .sort((left, right) => left.id - right.id);
  const renderedArrivals = new Set<number>();
  const entries: ConversationTimelineEntry<Message, Arrival>[] = [];
  const appendLegacyArrivals = () => {
    for (const arrival of legacyArrivals) {
      if (!renderedArrivals.has(arrival.id)) {
        entries.push({ kind: "arrival", arrival });
        renderedArrivals.add(arrival.id);
      }
    }
  };
  for (const message of messages) {
    const arrivalId = message.linkedDocumentArrival;
    if (message.author === "attachment") {
      if (arrivalId !== null && arrivalById.has(arrivalId)) {
        // Fully unlinked arrivals predate durable attachment anchors. Although
        // their exact position among old messages cannot be recovered, their
        // row ids preserve upload order and they must precede newer anchors.
        appendLegacyArrivals();
        entries.push({ kind: "arrival", arrival: arrivalById.get(arrivalId)! });
        renderedArrivals.add(arrivalId);
      }
      continue;
    }
    if (
      arrivalId !== null
      && arrivalById.has(arrivalId)
      && !renderedArrivals.has(arrivalId)
    ) {
      // Older Conversations predate durable attachment anchors. Their first linked
      // document reply is the earliest recoverable point in the timeline.
      entries.push({ kind: "arrival", arrival: arrivalById.get(arrivalId)! });
      renderedArrivals.add(arrivalId);
    }
    entries.push({ kind: "message", message });
  }
  for (const arrival of [...arrivals].sort((left, right) => left.id - right.id)) {
    if (!renderedArrivals.has(arrival.id)) {
      entries.push({ kind: "arrival", arrival });
    }
  }
  return entries;
}
