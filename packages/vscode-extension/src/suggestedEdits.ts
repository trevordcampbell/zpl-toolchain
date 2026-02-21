/**
 * Diagnostic-driven suggested edit actions for ZPL diagnostics.
 *
 * This layer only materializes explicit edits provided by diagnostic metadata.
 * It does not infer command-specific behavior.
 */

import * as vscode from "vscode";

type DiagnosticWithData = vscode.Diagnostic & { data?: unknown };

const SUGGESTED_EDIT_KIND_INSERT = "insert";
const SUGGESTED_EDIT_TEXT_PATTERN = /^[\^~][A-Za-z0-9@]{2}$/;
const SUGGESTED_EDIT_POSITION_VALUES = new Set([
  "document.end",
  "range.start",
  "range.end",
] as const);
type SuggestedEditInsertPosition = "document.end" | "range.start" | "range.end";

/**
 * Builds a safe CodeAction from `suggested_edit.*` diagnostic metadata.
 */
export function buildSuggestedEditAction(
  document: vscode.TextDocument,
  diagnostic: vscode.Diagnostic
): vscode.CodeAction | undefined {
  const kind = getDiagnosticDataValue(diagnostic, "suggested_edit.kind");
  if (kind !== SUGGESTED_EDIT_KIND_INSERT) {
    return undefined;
  }
  const text = getDiagnosticDataValue(diagnostic, "suggested_edit.text");
  if (!text || !SUGGESTED_EDIT_TEXT_PATTERN.test(text)) {
    return undefined;
  }
  const positionHint = getDiagnosticDataValue(diagnostic, "suggested_edit.position");
  if (
    !positionHint ||
    !SUGGESTED_EDIT_POSITION_VALUES.has(positionHint as SuggestedEditInsertPosition)
  ) {
    return undefined;
  }
  const insertPosition = resolveInsertPosition(
    diagnostic,
    positionHint as SuggestedEditInsertPosition,
    document
  );

  const edit = new vscode.WorkspaceEdit();
  edit.insert(document.uri, insertPosition, text);

  const title = getDiagnosticDataValue(diagnostic, "suggested_edit.title") ?? `Insert ${text}`;
  const action = new vscode.CodeAction(title, vscode.CodeActionKind.QuickFix);
  action.edit = edit;
  action.diagnostics = [diagnostic];
  action.isPreferred = true;
  return action;
}

function resolveInsertPosition(
  diagnostic: vscode.Diagnostic,
  positionHint: SuggestedEditInsertPosition,
  document: vscode.TextDocument
): vscode.Position {
  switch (positionHint) {
    case "document.end":
      return document.positionAt(document.getText().length);
    case "range.start":
      return diagnostic.range.start;
    case "range.end":
    default:
      return diagnostic.range.end;
  }
}

function getDiagnosticDataValue(diagnostic: vscode.Diagnostic, key: string): string | undefined {
  const data = (diagnostic as DiagnosticWithData).data;
  if (!data) {
    return undefined;
  }
  if (data instanceof Map) {
    const value = data.get(key);
    return typeof value === "string" ? value : undefined;
  }
  if (typeof data === "object") {
    const value = (data as Record<string, unknown>)[key];
    return typeof value === "string" ? value : undefined;
  }
  return undefined;
}
