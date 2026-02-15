import type { CommandDoc } from "./docsBundle";

export interface CompletionCommandContext {
  code: string;
  commandStart: number;
  commandEnd: number;
  argIndex: number | null;
}

export function resolveArgIndexWithSignature(
  entry: CommandDoc | undefined,
  lineText: string,
  context: CompletionCommandContext,
  cursorCharacter: number
): number | null {
  if (context.argIndex === null) {
    return null;
  }
  const joiner = entry?.signature?.joiner || ",";
  const argsStart = context.commandStart + context.code.length;
  const boundedCursor = Math.max(argsStart, Math.min(cursorCharacter, context.commandEnd));
  const commandSegment = lineText.slice(argsStart, boundedCursor);
  const tokens = commandSegment.length > 0 ? commandSegment.split(joiner) : [""];
  const rawIndex = Math.max(0, tokens.length - 1);

  const splitRule = entry?.signature?.splitRule;
  const splitIndex = splitRule?.paramIndex ?? null;
  const splitCounts = splitRule?.charCounts ?? [];
  const splitCount = splitCounts.length;
  if (splitIndex === null || splitCount <= 1) {
    return rawIndex;
  }
  if (rawIndex < splitIndex) {
    return rawIndex;
  }
  if (rawIndex > splitIndex) {
    return rawIndex + (splitCount - 1);
  }

  const compactToken = (tokens[splitIndex] ?? "").replace(/\s+/g, "");
  if (compactToken.length === 0) {
    return splitIndex;
  }
  let consumed = 0;
  for (let i = 0; i < splitCount; i += 1) {
    consumed += Math.max(1, splitCounts[i] ?? 1);
    if (compactToken.length <= consumed) {
      return splitIndex + i;
    }
  }
  return splitIndex + splitCount - 1;
}
