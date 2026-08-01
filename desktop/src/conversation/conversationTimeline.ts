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
  const renderedArrivals = new Set<number>();
  const entries: ConversationTimelineEntry<Message, Arrival>[] = [];
  for (const message of messages) {
    const arrivalId = message.linkedDocumentArrival;
    if (message.author === "attachment") {
      if (arrivalId !== null && arrivalById.has(arrivalId)) {
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
  for (const arrival of arrivals) {
    if (!renderedArrivals.has(arrival.id)) {
      entries.push({ kind: "arrival", arrival });
    }
  }
  return entries;
}
