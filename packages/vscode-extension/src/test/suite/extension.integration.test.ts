import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";
import * as assert from "node:assert/strict";

import * as vscode from "vscode";

const EXPLAIN_COMMAND_ID = "zplToolchain.explainDiagnostic";
const DEFAULT_PERF_BUDGET_MS = 8_000;

const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

async function waitFor<T>(
  fn: () => Promise<T> | T,
  isReady: (value: T) => boolean,
  timeoutMs = 12_000,
  intervalMs = 75
): Promise<T> {
  const start = Date.now();
  // eslint-disable-next-line no-constant-condition
  while (true) {
    const value = await fn();
    if (isReady(value)) {
      return value;
    }
    if (Date.now() - start > timeoutMs) {
      throw new Error("Timed out waiting for condition.");
    }
    await sleep(intervalMs);
  }
}

function repoRoot(): string {
  const value = process.env.ZPL_REPO_ROOT;
  if (!value) {
    throw new Error("ZPL_REPO_ROOT was not set for integration tests.");
  }
  return value;
}

async function createTempZplFile(name: string, content: string): Promise<vscode.Uri> {
  const dir = path.resolve(os.tmpdir(), "zpl-toolchain-extension-tests");
  await fs.mkdir(dir, { recursive: true });
  const filePath = path.resolve(dir, name);
  await fs.writeFile(filePath, content, "utf8");
  return vscode.Uri.file(filePath);
}

async function openZpl(uri: vscode.Uri): Promise<vscode.TextEditor> {
  const document = await vscode.workspace.openTextDocument(uri);
  return vscode.window.showTextDocument(document, { preview: false });
}

async function applyWholeText(editor: vscode.TextEditor, text: string): Promise<void> {
  const fullRange = new vscode.Range(
    editor.document.positionAt(0),
    editor.document.positionAt(editor.document.getText().length)
  );
  const ok = await editor.edit((editBuilder) => {
    editBuilder.replace(fullRange, text);
  });
  if (!ok) {
    throw new Error("Failed to apply editor update.");
  }
}

async function getDiagnostics(uri: vscode.Uri): Promise<readonly vscode.Diagnostic[]> {
  return vscode.languages
    .getDiagnostics(uri)
    .filter((diag) => diag.source === "zpl-toolchain");
}

async function applyFormattingEdits(
  uri: vscode.Uri,
  formatOptions: { tabSize: number; insertSpaces: boolean } = { tabSize: 2, insertSpaces: true }
): Promise<string> {
  const edits = (await vscode.commands.executeCommand(
    "vscode.executeFormatDocumentProvider",
    uri,
    formatOptions
  )) as vscode.TextEdit[];
  if (edits.length > 0) {
    const workspaceEdit = new vscode.WorkspaceEdit();
    for (const edit of edits) {
      workspaceEdit.replace(uri, edit.range, edit.newText);
    }
    const applied = await vscode.workspace.applyEdit(workspaceEdit);
    assert.ok(applied, "Expected formatting edits to apply.");
  }
  return (await vscode.workspace.openTextDocument(uri)).getText();
}

function completionLabel(item: vscode.CompletionItem): string {
  const raw = typeof item.label === "string" ? item.label : item.label.label;
  return raw.split(/\s+/, 1)[0] ?? raw;
}

function performanceBudgetMs(): number {
  const raw = process.env.ZPL_VSCODE_PERF_BUDGET_MS;
  if (!raw) {
    return DEFAULT_PERF_BUDGET_MS;
  }
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return DEFAULT_PERF_BUDGET_MS;
  }
  return parsed;
}

function buildLargeDocument(labelCount: number): string {
  const chunks: string[] = [];
  for (let i = 0; i < labelCount; i += 1) {
    const x = (i % 40) * 10;
    const y = (i % 30) * 10;
    chunks.push(`^XA\n^FO${x},${y}^FDLABEL-${i.toString().padStart(4, "0")}^FS\n^XZ\n`);
  }
  return chunks.join("");
}

suite("VS Code extension integration", () => {
  const created: vscode.Uri[] = [];
  let extension: vscode.Extension<unknown> | undefined;

  suiteSetup(async () => {
    extension = vscode.extensions.getExtension("trevordcampbell.zpl-toolchain");
    assert.ok(extension, "Expected extension to be discoverable by identifier.");
    if (!extension.isActive) {
      await extension.activate();
    }
  });

  teardown(async () => {
    for (const uri of created) {
      try {
        await vscode.workspace.fs.delete(uri, { useTrash: false });
      } catch {
        // Ignore cleanup failures for ephemeral fixtures.
      }
    }
    created.length = 0;
  });

  test("diagnostics converge after rapid edits (race regression)", async () => {
    const uri = await createTempZplFile(
      "race-diagnostics.zpl",
      "^XA\n^FO10,10^FDok^FS\n^XZ\n"
    );
    created.push(uri);

    await vscode.workspace
      .getConfiguration("zplToolchain", uri)
      .update("diagnostics.debounceMs", 25, vscode.ConfigurationTarget.Workspace);

    const editor = await openZpl(uri);

    await applyWholeText(editor, "^XA\n^FO10,10^FDoops^FS\n");
    await waitFor(
      () => getDiagnostics(uri),
      (diags) => diags.some((diag) => diag.severity === vscode.DiagnosticSeverity.Error)
    );

    const valid = "^XA\n^FO10,10^FDok^FS\n^XZ\n";
    const invalid = "^XA\n^FO10,10^FDoops^FS\n";
    for (let i = 0; i < 12; i += 1) {
      await applyWholeText(editor, i % 2 === 0 ? invalid : valid);
    }
    await applyWholeText(editor, valid);

    const finalDiagnostics = await waitFor(
      () => getDiagnostics(uri),
      (diags) => diags.every((diag) => diag.severity !== vscode.DiagnosticSeverity.Error)
    );
    assert.ok(
      finalDiagnostics.every((diag) => diag.severity !== vscode.DiagnosticSeverity.Error),
      "Expected final diagnostics to be free of errors."
    );
  });

  test("explain command wiring appears through code actions", async () => {
    const uri = await createTempZplFile("explain-code-action.zpl", "^XA\n^FO10,10^FDoops^FS\n");
    created.push(uri);

    const editor = await openZpl(uri);
    const diagnostics = await waitFor(
      () => getDiagnostics(uri),
      (diags) => diags.length > 0
    );
    const target = diagnostics[0];
    assert.ok(target, "Expected at least one diagnostic to drive code actions.");

    const commandIds = await vscode.commands.getCommands(true);
    assert.ok(
      commandIds.includes(EXPLAIN_COMMAND_ID),
      `Expected command registration for ${EXPLAIN_COMMAND_ID}.`
    );

    const actions = (await vscode.commands.executeCommand(
      "vscode.executeCodeActionProvider",
      editor.document.uri,
      target.range
    )) as Array<vscode.Command | vscode.CodeAction>;

    const hasExplain = actions.some((item) => {
      if ("command" in item && typeof item.command === "string") {
        return item.command === EXPLAIN_COMMAND_ID;
      }
      if ("command" in item && typeof item.command !== "string" && item.command) {
        return item.command.command === EXPLAIN_COMMAND_ID;
      }
      return false;
    });

    assert.ok(hasExplain, "Expected explain diagnostic code action wiring.");
  });

  test("hover resolves command metadata", async () => {
    const uri = await createTempZplFile("hover-docs.zpl", "^XA\n^FO10,10^FDdata^FS\n^XZ\n");
    created.push(uri);

    const editor = await openZpl(uri);
    const hovers = (await vscode.commands.executeCommand(
      "vscode.executeHoverProvider",
      editor.document.uri,
      new vscode.Position(1, 1)
    )) as vscode.Hover[];

    assert.ok(hovers.length > 0, "Expected hover results for ^FO.");
    const content = hovers
      .flatMap((hover) => hover.contents)
      .map((part) => (part instanceof vscode.MarkdownString ? part.value : String(part)))
      .join("\n");
    assert.match(content, /\^FO/, "Expected hover content to include command opcode.");
  });

  test("command completions appear for new command prefix", async () => {
    const uri = await createTempZplFile("completion-command-prefix.zpl", "^XA\n^f\n^XZ\n");
    created.push(uri);

    const editor = await openZpl(uri);
    const completionList = (await vscode.commands.executeCommand(
      "vscode.executeCompletionItemProvider",
      editor.document.uri,
      new vscode.Position(1, 2)
    )) as vscode.CompletionList;

    assert.ok(completionList.items.length > 0, "Expected command completion items for ^f.");
    assert.ok(
      completionList.items.some((item) => completionLabel(item) === "^FO"),
      "Expected ^FO in command completions for ^f prefix."
    );
    const fo = completionList.items.find((item) => completionLabel(item) === "^FO");
    assert.ok(fo, "Expected ^FO completion item.");
    const foRow = typeof fo.label === "string" ? fo.label : fo.label.label;
    assert.match(foRow, /\[FORMAT\/FLD\]/, "Expected ^FO row to include compact category/scope badge.");
    assert.match(foRow, /field origin/i, "Expected ^FO row to include descriptive name.");
  });

  test("alias completion entries resolve rich docs for ^FV", async () => {
    const uri = await createTempZplFile("completion-alias-fv.zpl", "^XA\n^F\n^XZ\n");
    created.push(uri);

    const editor = await openZpl(uri);
    const completionList = (await vscode.commands.executeCommand(
      "vscode.executeCompletionItemProvider",
      editor.document.uri,
      new vscode.Position(1, 2)
    )) as vscode.CompletionList;

    const fv = completionList.items.find((item) => completionLabel(item) === "^FV");
    assert.ok(fv, "Expected ^FV in command completions for ^F prefix.");
    assert.notEqual(
      fv.detail,
      "ZPL command",
      "Expected ^FV completion to show resolved docs, not generic fallback."
    );
  });

  test("hover resolves alias metadata for ^A0", async () => {
    const uri = await createTempZplFile("hover-a0-alias.zpl", "^XA\n^A0N,30,30\n^XZ\n");
    created.push(uri);

    const editor = await openZpl(uri);
    const hovers = (await vscode.commands.executeCommand(
      "vscode.executeHoverProvider",
      editor.document.uri,
      new vscode.Position(1, 2)
    )) as vscode.Hover[];

    assert.ok(hovers.length > 0, "Expected hover results for ^A0.");
    const content = hovers
      .flatMap((hover) => hover.contents)
      .map((part) => (part instanceof vscode.MarkdownString ? part.value : String(part)))
      .join("\n");
    assert.match(content, /\^A0/, "Expected hover title to include ^A0.");
    assert.match(content, /Arguments:/, "Expected hover to include argument metadata.");
  });

  test("hover resolves alias metadata for ^FV", async () => {
    const uri = await createTempZplFile("hover-fv-alias.zpl", "^XA\n^FO10,10^FVVALUE^FS\n^XZ\n");
    created.push(uri);

    const editor = await openZpl(uri);
    const hovers = (await vscode.commands.executeCommand(
      "vscode.executeHoverProvider",
      editor.document.uri,
      new vscode.Position(1, 11)
    )) as vscode.Hover[];

    assert.ok(hovers.length > 0, "Expected hover results for ^FV.");
    const content = hovers
      .flatMap((hover) => hover.contents)
      .map((part) => (part instanceof vscode.MarkdownString ? part.value : String(part)))
      .join("\n");
    assert.match(content, /\^FV/, "Expected hover title to include ^FV.");
    assert.match(
      content,
      /variable data/i,
      "Expected hover to include resolved alias documentation content."
    );
  });

  test("hover-only notes do not pollute Problems diagnostics", async () => {
    const uri = await createTempZplFile("hover-only-note.zpl", "^XA\n^BY2,3,80\n^XZ\n");
    created.push(uri);

    const editor = await openZpl(uri);
    const diagnostics = await waitFor(
      () => getDiagnostics(uri),
      () => true
    );
    assert.equal(
      diagnostics.filter((diag) => String(diag.code) === "ZPL3001").length,
      0,
      "Expected hover-only note diagnostics to be omitted from Problems."
    );

    const hovers = (await vscode.commands.executeCommand(
      "vscode.executeHoverProvider",
      editor.document.uri,
      new vscode.Position(1, 2)
    )) as vscode.Hover[];
    const content = hovers
      .flatMap((hover) => hover.contents)
      .map((part) => (part instanceof vscode.MarkdownString ? part.value : String(part)))
      .join("\n");
    assert.match(content, /Additional Notes/i, "Expected hover to include additional notes section.");
    assert.match(
      content,
      /\^BY sets defaults for subsequent barcode commands/i,
      "Expected hover to surface explanatory note content."
    );
  });

  test("formatting is idempotent on sample labels", async () => {
    const samplesDir = path.resolve(repoRoot(), "samples");
    const sampleNames = [
      "warehouse_label.zpl",
      "shipping_label.zpl",
      "product_label.zpl",
      "compliance_label.zpl",
      "usps_surepost_sample.zpl",
    ];

    for (const sampleName of sampleNames) {
      const samplePath = path.resolve(samplesDir, sampleName);
      const source = await fs.readFile(samplePath, "utf8");
      const uri = await createTempZplFile(`format-${sampleName}`, source);
      created.push(uri);

      const document = await vscode.workspace.openTextDocument(uri);
      const formatOptions = { tabSize: 2, insertSpaces: true };
      const edits1 = (await vscode.commands.executeCommand(
        "vscode.executeFormatDocumentProvider",
        document.uri,
        formatOptions
      )) as vscode.TextEdit[];

      if (edits1.length > 0) {
        const workspaceEdit = new vscode.WorkspaceEdit();
        for (const edit of edits1) {
          workspaceEdit.replace(document.uri, edit.range, edit.newText);
        }
        const applied = await vscode.workspace.applyEdit(workspaceEdit);
        assert.ok(applied, `Expected first format edits to apply for ${sampleName}.`);
      }

      const formattedOnce = (await vscode.workspace.openTextDocument(uri)).getText();
      const edits2 = (await vscode.commands.executeCommand(
        "vscode.executeFormatDocumentProvider",
        uri,
        formatOptions
      )) as vscode.TextEdit[];

      if (edits2.length === 0) {
        continue;
      }

      const secondEdit = new vscode.WorkspaceEdit();
      for (const edit of edits2) {
        secondEdit.replace(uri, edit.range, edit.newText);
      }
      const appliedSecond = await vscode.workspace.applyEdit(secondEdit);
      assert.ok(appliedSecond, `Expected second format edits to apply for ${sampleName}.`);

      const formattedTwice = (await vscode.workspace.openTextDocument(uri)).getText();
      assert.equal(
        formattedTwice,
        formattedOnce,
        `Expected idempotent formatting on ${sampleName}.`
      );
    }
  });

  test("formatting respects non-default compaction and comment placement settings", async () => {
    const uri = await createTempZplFile(
      "format-non-default-settings.zpl",
      "^XA\n^FO30,30\n^A0N,35,35\n^FDWIDGET-3000\n^FS\n^PW812\n; set print width\n^XZ\n"
    );
    created.push(uri);
    await openZpl(uri);

    const config = vscode.workspace.getConfiguration("zplToolchain", uri);
    const previousCompaction = config.get("format.compaction");
    const previousCommentPlacement = config.get("format.commentPlacement");

    try {
      await config.update("format.compaction", "none", vscode.ConfigurationTarget.Workspace);
      await config.update(
        "format.commentPlacement",
        "line",
        vscode.ConfigurationTarget.Workspace
      );

      const formatted = await applyFormattingEdits(uri);
      assert.match(formatted, /\^FO30,30\n\^A0N,35,35\n\^FDWIDGET-3000\n\^FS/);
      assert.match(formatted, /\^PW812\n; set print width/);
    } finally {
      await config.update(
        "format.compaction",
        previousCompaction,
        vscode.ConfigurationTarget.Workspace
      );
      await config.update(
        "format.commentPlacement",
        previousCommentPlacement,
        vscode.ConfigurationTarget.Workspace
      );
    }
  });

  test("large-document diagnostics stay within latency budget", async () => {
    const budgetMs = performanceBudgetMs();
    const largeDocument = buildLargeDocument(400);
    const invalidLargeDocument = `${largeDocument}\n^XA\n^FO10,10^FDUNTERMINATED^FS\n`;
    const uri = await createTempZplFile("large-perf.zpl", largeDocument);
    created.push(uri);

    await vscode.workspace
      .getConfiguration("zplToolchain", uri)
      .update("diagnostics.debounceMs", 0, vscode.ConfigurationTarget.Workspace);

    const editor = await openZpl(uri);
    await applyWholeText(editor, invalidLargeDocument);
    await waitFor(
      () => getDiagnostics(uri),
      (diags) => diags.some((diag) => diag.severity === vscode.DiagnosticSeverity.Error),
      budgetMs,
      25
    );

    const start = Date.now();
    await applyWholeText(editor, largeDocument);
    const diagnostics = await waitFor(
      () => getDiagnostics(uri),
      (diags) => diags.every((diag) => diag.severity !== vscode.DiagnosticSeverity.Error),
      budgetMs,
      25
    );
    const elapsed = Date.now() - start;

    assert.ok(
      elapsed <= budgetMs,
      `Expected diagnostics latency <= ${budgetMs}ms for large doc, got ${elapsed}ms`
    );
    assert.ok(
      diagnostics.every((diag) => diag.severity !== vscode.DiagnosticSeverity.Error),
      "Large-document fixture should remain valid and avoid error diagnostics."
    );
  });
});
