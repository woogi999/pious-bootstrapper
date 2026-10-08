// FastFlags: reading them however people paste them.
const asText = (v: unknown) => (typeof v === "string" ? v : JSON.stringify(v));

/** Reads FastFlags from what someone pasted or opened: a JSON object
 * (Bloxstrap's ClientAppSettings.json, a flag list, with or without
 * trailing commas or comments), or one `Name=Value` / `Name: Value` per
 * line. Throws with a readable message when there's nothing to read. */
export function parseFlags(text: string): Record<string, string> {
  const trimmed = text.trim();
  if (!trimmed) throw new Error("Paste some FastFlags first.");
  // JSON, tolerating comments and trailing commas (common in shared lists).
  if (trimmed.startsWith("{") || trimmed.startsWith('"')) {
    const body = trimmed.startsWith("{") ? trimmed : `{${trimmed}}`;
    const relaxed = body
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(/^\s*\/\/.*$/gm, "")
      .replace(/,\s*([}\]])/g, "$1");
    let parsed: unknown;
    try {
      parsed = JSON.parse(relaxed);
    } catch (e) {
      throw new Error(`That isn't valid JSON: ${(e as Error).message}`);
    }
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("FastFlags must be a JSON object of names and values.");
    return Object.fromEntries(Object.entries(parsed as Record<string, unknown>).map(([k, v]) => [k.trim(), asText(v)]));
  }
  // One flag per line.
  const flags: Record<string, string> = {};
  for (const raw of trimmed.split(/\r?\n/)) {
    const line = raw.trim().replace(/,$/, "");
    if (!line || line.startsWith("#") || line.startsWith("//")) continue;
    const match = /^"?([A-Za-z_][\w]*)"?\s*[:=]\s*"?(.*?)"?$/.exec(line);
    if (!match) throw new Error(`Couldn't read this line: ${line}`);
    flags[match[1]] = match[2];
  }
  return flags;
}
