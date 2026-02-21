import * as assert from "node:assert/strict";

import * as vscode from "vscode";

import { resolveArgIndexWithSignature, type CompletionCommandContext } from "../../completionArgs";
import type { CoreDiagnostic } from "../../coreApi";
import { getDiagnosticContextValue, partitionCoreIssues } from "../../diagnosticRouting";
import type { CommandDoc } from "../../docsBundle";
import { buildSuggestedEditAction } from "../../suggestedEdits";

type DiagnosticWithData = vscode.Diagnostic & { data?: unknown };

function issue(input: Partial<CoreDiagnostic>): CoreDiagnostic {
  return {
    id: input.id ?? "ZPL0000",
    severity: input.severity ?? "info",
    message: input.message ?? "test",
    span: input.span,
    context: input.context,
  };
}

suite("extension logic unit tests", () => {
  test("diagnostic context value supports plain object and Map", () => {
    const objectIssue = issue({
      context: { audience: "contextual" },
    });
    const mapIssue = issue({
      context: new Map([["audience", "problem"]]),
    });

    assert.equal(getDiagnosticContextValue(objectIssue, "audience"), "contextual");
    assert.equal(getDiagnosticContextValue(mapIssue, "audience"), "problem");
  });

  test("partition routes contextual notes away from Problems", () => {
    const contextualInfo = issue({
      id: "ZPL3001",
      severity: "info",
      context: new Map([["audience", "contextual"]]),
    });
    const regularWarn = issue({
      id: "ZPL2000",
      severity: "warn",
      context: { audience: "problem" },
    });
    const fallbackInfo = issue({
      id: "ZPL3002",
      severity: "info",
    });
    const explicitProblemInfo = issue({
      id: "ZPL3003",
      severity: "info",
      context: { audience: "problem" },
    });

    const { hoverOnlyIssues, problemIssues } = partitionCoreIssues([
      contextualInfo,
      regularWarn,
      fallbackInfo,
      explicitProblemInfo,
    ]);

    assert.deepEqual(
      hoverOnlyIssues.map((diag) => diag.id),
      ["ZPL3001", "ZPL3002"]
    );
    assert.deepEqual(
      problemIssues.map((diag) => diag.id),
      ["ZPL2000", "ZPL3003"]
    );
  });

  test("split-rule arg resolution handles compact and comma-delimited positions", () => {
    const entry: CommandDoc = {
      signature: {
        joiner: ",",
        splitRule: {
          paramIndex: 0,
          charCounts: [1, 1],
        },
      },
    };
    const lineText = "^A0N,30,30";
    const context: CompletionCommandContext = {
      code: "^A",
      commandStart: 0,
      commandEnd: lineText.length,
      argIndex: 0,
    };

    assert.equal(resolveArgIndexWithSignature(entry, lineText, context, 2), 0);
    assert.equal(resolveArgIndexWithSignature(entry, lineText, context, 3), 0);
    assert.equal(resolveArgIndexWithSignature(entry, lineText, context, 4), 1);
    assert.equal(resolveArgIndexWithSignature(entry, lineText, context, 5), 2);
    assert.equal(resolveArgIndexWithSignature(entry, lineText, context, lineText.length), 3);
  });

  suite("diagnostic suggested edits", () => {
    test("buildSuggestedEditAction returns undefined without suggested_edit metadata", async () => {
      const doc = await vscode.workspace.openTextDocument({
        content: "^XA^FO10,10^FDok^FS",
        language: "zpl",
      });
      const diag = new vscode.Diagnostic(
        new vscode.Range(0, 0, 0, 5),
        "Test",
        vscode.DiagnosticSeverity.Error
      );
      diag.code = "ZPL1101";

      const action = buildSuggestedEditAction(doc, diag);
      assert.equal(action, undefined);
    });

    test("buildSuggestedEditAction inserts ^XZ at document end", async () => {
      const content = "^XA^FO10,10^FDok^FS";
      const doc = await vscode.workspace.openTextDocument({
        content,
        language: "zpl",
      });
      const diag = new vscode.Diagnostic(
        new vscode.Range(
          doc.positionAt(content.length),
          doc.positionAt(content.length)
        ),
        "Missing terminator (^XZ)",
        vscode.DiagnosticSeverity.Error
      );
      (diag as DiagnosticWithData).data = {
        "suggested_edit.kind": "insert",
        "suggested_edit.text": "^XZ",
        "suggested_edit.position": "document.end",
        "suggested_edit.title": "Insert ^XZ (label terminator)",
      };

      const action = buildSuggestedEditAction(doc, diag);
      assert.ok(action, "Expected a suggested edit action");
      assert.equal(action.title, "Insert ^XZ (label terminator)");
      assert.ok(action.edit, "Expected edit on action");
      const entries = action.edit!.entries();
      assert.equal(entries.length, 1);
      const [uri, edits] = entries[0];
      assert.equal(edits.length, 1);
      const textEdit = edits[0] as vscode.TextEdit;
      assert.equal(textEdit.newText, "^XZ");
      assert.equal(
        doc.offsetAt(textEdit.range.start),
        content.length,
        "Insert position should be at document end"
      );
    });

    test("buildSuggestedEditAction inserts ^FS at range start", async () => {
      const content = "^XA^FO10,10^FDHello^XZ";
      const doc = await vscode.workspace.openTextDocument({
        content,
        language: "zpl",
      });
      const xzStart = content.indexOf("^XZ");
      const diag = new vscode.Diagnostic(
        new vscode.Range(
          doc.positionAt(xzStart),
          doc.positionAt(xzStart + 3)
        ),
        "Missing field separator (^FS) before ^XZ",
        vscode.DiagnosticSeverity.Error
      );
      (diag as DiagnosticWithData).data = {
        "suggested_edit.kind": "insert",
        "suggested_edit.text": "^FS",
        "suggested_edit.position": "range.start",
        "suggested_edit.title": "Insert ^FS (field separator)",
      };

      const action = buildSuggestedEditAction(doc, diag);
      assert.ok(action, "Expected a suggested edit action");
      assert.equal(action.title, "Insert ^FS (field separator)");
      assert.ok(action.edit, "Expected edit on action");
      const entries = action.edit!.entries();
      assert.equal(entries.length, 1);
      const [, edits] = entries[0];
      const textEdit = edits[0] as vscode.TextEdit;
      assert.equal(textEdit.newText, "^FS");
      assert.equal(
        doc.offsetAt(textEdit.range.start),
        xzStart,
        "Insert position should be before ^XZ"
      );
    });

    test("buildSuggestedEditAction inserts ^FS at range end", async () => {
      const content = "^XA^FO10,10^FDHello";
      const doc = await vscode.workspace.openTextDocument({
        content,
        language: "zpl",
      });
      const fieldStart = content.indexOf("Hello");
      const diag = new vscode.Diagnostic(
        new vscode.Range(
          doc.positionAt(fieldStart),
          doc.positionAt(content.length)
        ),
        "Missing field separator (^FS) before end of input",
        vscode.DiagnosticSeverity.Error
      );
      (diag as DiagnosticWithData).data = {
        "suggested_edit.kind": "insert",
        "suggested_edit.text": "^FS",
        "suggested_edit.position": "range.end",
        "suggested_edit.title": "Insert ^FS (field separator)",
      };

      const action = buildSuggestedEditAction(doc, diag);
      assert.ok(action, "Expected a suggested edit action");
      assert.equal(action.title, "Insert ^FS (field separator)");
      assert.ok(action.edit, "Expected edit on action");
      const [, edits] = action.edit!.entries()[0];
      const textEdit = edits[0] as vscode.TextEdit;
      assert.equal(textEdit.newText, "^FS");
      assert.equal(
        doc.offsetAt(textEdit.range.start),
        content.length,
        "Insert position should be at document end"
      );
    });
  });
});
