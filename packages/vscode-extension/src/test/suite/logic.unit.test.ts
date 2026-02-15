import * as assert from "node:assert/strict";

import { resolveArgIndexWithSignature, type CompletionCommandContext } from "../../completionArgs";
import type { CoreDiagnostic } from "../../coreApi";
import { getDiagnosticContextValue, partitionCoreIssues } from "../../diagnosticRouting";
import type { CommandDoc } from "../../docsBundle";

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
});
