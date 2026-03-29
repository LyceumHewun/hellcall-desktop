import { StratagemDirection, StratagemItem } from "../types/stratagems";

const DIRECTION_SET = new Set<StratagemDirection>([
  "UP",
  "DOWN",
  "LEFT",
  "RIGHT",
]);

export function getDirectionalSequence(sequence: string[]): StratagemDirection[] {
  return sequence.filter((key): key is StratagemDirection =>
    DIRECTION_SET.has(key as StratagemDirection),
  );
}

export function encodeStratagemCommandId(
  command: readonly StratagemDirection[],
): string {
  return btoa(command.join(","));
}

export function findExactStratagemMatch(
  items: StratagemItem[],
  sequence: readonly string[],
): StratagemItem | null {
  const directionalSequence = getDirectionalSequence([...sequence]);
  if (directionalSequence.length === 0) {
    return null;
  }

  const id = encodeStratagemCommandId(directionalSequence);
  return items.find((item) => item.id === id) ?? null;
}

export function findStratagemPrefixMatches(
  items: StratagemItem[],
  sequence: readonly string[],
): StratagemItem[] {
  const directionalSequence = getDirectionalSequence([...sequence]);
  if (directionalSequence.length === 0) {
    return [];
  }

  return items.filter((item) =>
    directionalSequence.every((direction, index) => item.command[index] === direction),
  );
}
