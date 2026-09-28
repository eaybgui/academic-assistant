/** Parses a comma-separated alias input into a trimmed, non-empty array. */
export function parseAliases(raw: string): string[] {
  return raw
    .split(',')
    .map((alias) => alias.trim())
    .filter((alias) => alias.length > 0);
}
