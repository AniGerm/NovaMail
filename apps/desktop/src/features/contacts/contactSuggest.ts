import type { RecipientSuggestion } from "@/shared/api/types";

export function splitPersonName(name: string): {
  givenName: string;
  familyName: string;
  displayName: string;
} {
  const displayName = name.trim().replace(/\s+/g, " ");
  const parts = displayName.split(" ").filter(Boolean);
  if (parts.length === 0) {
    return { givenName: "", familyName: "", displayName: "" };
  }
  if (parts.length === 1) {
    return { givenName: "", familyName: parts[0] ?? "", displayName };
  }
  return {
    givenName: parts.slice(0, -1).join(" "),
    familyName: parts[parts.length - 1] ?? "",
    displayName,
  };
}

export function mergeEmailList(current: string, email: string): string {
  const next = email.trim();
  if (!next) return current;
  const items = current
    .split(",")
    .map((part) => part.trim())
    .filter(Boolean);
  if (items.some((item) => item.toLowerCase() === next.toLowerCase())) {
    return items.join(", ");
  }
  items.push(next);
  return items.join(", ");
}

export function suggestQueryToken(value: string): string {
  const parts = value.split(",");
  return (parts[parts.length - 1] ?? "").trim();
}

/** Fields a history hit can fill. Existing phones and notes stay. */
export function historyFieldPatch(
  suggestion: Pick<RecipientSuggestion, "name" | "email">,
): {
  givenName: string;
  familyName: string;
  displayName: string;
  email: string;
} {
  const names = splitPersonName(suggestion.name ?? "");
  return { ...names, email: suggestion.email.trim() };
}
