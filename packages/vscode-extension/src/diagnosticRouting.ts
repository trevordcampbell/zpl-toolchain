import type { CoreDiagnostic } from "./coreApi";

export function getDiagnosticContextValue(
  issue: CoreDiagnostic,
  key: string
): string | undefined {
  const context = issue.context;
  if (!context) {
    return undefined;
  }
  if (context instanceof Map) {
    const value = context.get(key);
    return typeof value === "string" ? value : undefined;
  }
  if (typeof context === "object") {
    const value = (context as Record<string, unknown>)[key];
    return typeof value === "string" ? value : undefined;
  }
  return undefined;
}

export function partitionCoreIssues(issues: CoreDiagnostic[]): {
  problemIssues: CoreDiagnostic[];
  hoverOnlyIssues: CoreDiagnostic[];
} {
  const problemIssues: CoreDiagnostic[] = [];
  const hoverOnlyIssues: CoreDiagnostic[] = [];
  for (const issue of issues) {
    const audience = getDiagnosticContextValue(issue, "audience");
    if (
      audience === "contextual" ||
      (issue.severity === "info" && audience !== "problem")
    ) {
      hoverOnlyIssues.push(issue);
      continue;
    }
    problemIssues.push(issue);
  }
  return { problemIssues, hoverOnlyIssues };
}
