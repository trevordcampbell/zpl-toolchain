import * as path from "node:path";
import * as vscode from "vscode";

import { getCoreApi, type CoreDiagnostic } from "./coreApi";
import { resolveArgIndexWithSignature } from "./completionArgs";
import { partitionCoreIssues } from "./diagnosticRouting";
import {
  loadDocsBundle,
  type CommandDoc,
  type DocsArg,
  type DocsBundle,
} from "./docsBundle";
import { NoopRendererBridge, type RendererBridge } from "./rendererBridge";
import { Utf8LineIndex } from "./utf8LineIndex";

const DIAGNOSTIC_SOURCE = "zpl-toolchain";
const EXPLAIN_COMMAND_ID = "zplToolchain.explainDiagnostic";
const APPLY_THEME_PRESET_COMMAND_ID = "zplToolchain.applyThemePreset";
const COMMAND_PREFIX_REGEX = /[\^~][A-Za-z0-9@]*$/;
const COMMAND_TOKEN_REGEX_SOURCE = "\\^A[0-9]|\\^A(?=[A-Za-z])|[\\^~][A-Za-z0-9@]{1,2}";
const COMMAND_COMPLETION_TRIGGER_CHARACTERS = [
  "^",
  "~",
  ",",
  ..."ABCDEFGHIJKLMNOPQRSTUVWXYZ",
  ..."abcdefghijklmnopqrstuvwxyz",
  ..."0123456789",
  "@",
] as const;
const FIELD_OPENING_COMMANDS = new Set(["^FO", "^FT", "^FM", "^FN"]);

type ThemePreset = "custom" | "default" | "high-contrast" | "minimal";
type FormatCompaction = "none" | "field";
type CommentPlacement = "inline" | "line";

type CommandContext = {
  code: string;
  argIndex: number | null;
  commandStart: number;
  commandEnd: number;
};

type CompletionContextState = {
  inLabel: boolean;
  inField: boolean;
};

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const diagnostics = vscode.languages.createDiagnosticCollection(DIAGNOSTIC_SOURCE);
  context.subscriptions.push(diagnostics);
  const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  statusBar.name = "ZPL Toolchain";
  statusBar.command = APPLY_THEME_PRESET_COMMAND_ID;
  context.subscriptions.push(statusBar);

  let docsBundlePromise: Promise<DocsBundle> | undefined;
  const rendererBridge: RendererBridge = new NoopRendererBridge();

  const refreshTimers = new Map<string, NodeJS.Timeout>();
  const validationGenerations = new Map<string, number>();
  const lineIndexCache = new Map<string, { version: number; index: Utf8LineIndex }>();
  const hoverNoteIssues = new Map<string, { version: number; issues: CoreDiagnostic[] }>();
  context.subscriptions.push({
    dispose: () => {
      for (const timer of refreshTimers.values()) {
        clearTimeout(timer);
      }
      refreshTimers.clear();
      validationGenerations.clear();
      lineIndexCache.clear();
      hoverNoteIssues.clear();
    },
  });

  const updateStatusBar = (): void => {
    const editor = vscode.window.activeTextEditor;
    if (!editor || !isZplDocument(editor.document)) {
      statusBar.hide();
      return;
    }
    const docDiagnostics = diagnostics.get(editor.document.uri);
    const errors = docDiagnostics?.filter((diag) => diag.severity === vscode.DiagnosticSeverity.Error)
      .length ?? 0;
    const warnings = docDiagnostics?.filter((diag) => diag.severity === vscode.DiagnosticSeverity.Warning)
      .length ?? 0;
    if (errors > 0) {
      statusBar.text = `$(error) ZPL Toolchain (${errors} errors)`;
      statusBar.tooltip = "ZPL Toolchain active - click to apply theme preset";
    } else if (warnings > 0) {
      statusBar.text = `$(warning) ZPL Toolchain (${warnings} warnings)`;
      statusBar.tooltip = "ZPL Toolchain active - click to apply theme preset";
    } else {
      statusBar.text = "$(check) ZPL Toolchain";
      statusBar.tooltip = "ZPL Toolchain active - click to apply theme preset";
    }
    statusBar.show();
  };

  const refreshDiagnostics = (document: vscode.TextDocument): void => {
    if (!isZplDocument(document)) {
      diagnostics.delete(document.uri);
      return;
    }

    const key = document.uri.toString();
    const existing = refreshTimers.get(key);
    if (existing) {
      clearTimeout(existing);
    }

    const generation = (validationGenerations.get(key) ?? 0) + 1;
    validationGenerations.set(key, generation);
    const debounceMs = getDebounceMs();
    const timer = setTimeout(async () => {
      refreshTimers.delete(key);
      if (validationGenerations.get(key) !== generation) {
        return;
      }
      try {
        const liveDocument = vscode.workspace.textDocuments.find(
          (candidate) => candidate.uri.toString() === key
        );
        if (!liveDocument) {
          diagnostics.delete(vscode.Uri.parse(key));
          return;
        }

        const core = await getCoreApi();
        if (validationGenerations.get(key) !== generation) {
          return;
        }
        const profileJson = getProfileJson();
        const result = core.validate(liveDocument.getText(), profileJson || undefined);
        const partitioned = partitionCoreIssues(result.issues);

        if (validationGenerations.get(key) !== generation) {
          return;
        }

        hoverNoteIssues.set(liveDocument.uri.toString(), {
          version: liveDocument.version,
          issues: partitioned.hoverOnlyIssues,
        });
        diagnostics.set(
          liveDocument.uri,
          toVsDiagnostics(liveDocument, partitioned.problemIssues, lineIndexCache)
        );
        updateStatusBar();
      } catch (error) {
        if (validationGenerations.get(key) !== generation) {
          return;
        }
        const message =
          error instanceof Error ? error.message : "unknown validation error";
        const fallback = new vscode.Diagnostic(
          new vscode.Range(new vscode.Position(0, 0), new vscode.Position(0, 0)),
          `Validation failed: ${message}`,
          vscode.DiagnosticSeverity.Error
        );
        fallback.source = DIAGNOSTIC_SOURCE;
        diagnostics.set(vscode.Uri.parse(key), [fallback]);
        updateStatusBar();
      }
    }, debounceMs);

    refreshTimers.set(key, timer);
  };

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((doc) => refreshDiagnostics(doc)),
    vscode.workspace.onDidChangeTextDocument((event) => refreshDiagnostics(event.document)),
    vscode.workspace.onDidSaveTextDocument((doc) => refreshDiagnostics(doc)),
    vscode.workspace.onDidCloseTextDocument((doc) => {
      const key = doc.uri.toString();
      const timer = refreshTimers.get(key);
      if (timer) {
        clearTimeout(timer);
        refreshTimers.delete(key);
      }
      validationGenerations.delete(key);
      lineIndexCache.delete(key);
      diagnostics.delete(doc.uri);
      hoverNoteIssues.delete(key);
      updateStatusBar();
    })
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (!event.affectsConfiguration("zplToolchain")) {
        return;
      }
      for (const document of vscode.workspace.textDocuments) {
        if (isZplDocument(document)) {
          refreshDiagnostics(document);
        }
      }
      if (event.affectsConfiguration("zplToolchain.themePreset")) {
        void applyThemePreset(getThemePreset(), "auto");
      }
      updateStatusBar();
    })
  );
  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor(() => updateStatusBar())
  );

  for (const document of vscode.workspace.textDocuments) {
    refreshDiagnostics(document);
  }
  updateStatusBar();

  context.subscriptions.push(
    vscode.languages.registerDocumentFormattingEditProvider("zpl", {
      provideDocumentFormattingEdits: async (document, _options, token) => {
        if (token.isCancellationRequested) {
          return [];
        }
        const core = await getCoreApi();
        if (token.isCancellationRequested) {
          return [];
        }
        const indentStyle = getIndentStyle();
        const finalText = core.format(
          document.getText(),
          indentStyle,
          getFormatCompaction(),
          getCommentPlacement()
        );
        const fullRange = new vscode.Range(
          document.positionAt(0),
          document.positionAt(document.getText().length)
        );
        return [vscode.TextEdit.replace(fullRange, finalText)];
      },
    })
  );

  context.subscriptions.push(
    vscode.languages.registerHoverProvider("zpl", {
      provideHover: async (document, position, token) => {
        if (token.isCancellationRequested) {
          return undefined;
        }
        if (!isHoverEnabled()) {
          return undefined;
        }
        const commandContext = extractCommandContextAtPosition(document, position);
        if (!commandContext) {
          return undefined;
        }
        const code = commandContext.code;
        const lineText = document.lineAt(position.line).text;

        docsBundlePromise ??= initializeDocsBundle(context);
        const docsBundle = await docsBundlePromise;
        if (token.isCancellationRequested) {
          return undefined;
        }
        const entry = resolveCommandDoc(docsBundle, code);
        if (!entry) {
          return undefined;
        }

        const markdown = new vscode.MarkdownString();
        markdown.appendMarkdown(`### ${code}\n\n`);
        if (entry.docs) {
          markdown.appendMarkdown(`${entry.docs}\n\n`);
        }
        if (entry.formatTemplate) {
          markdown.appendCodeblock(entry.formatTemplate, "text");
        }
        const argIndex = resolveArgIndexWithSignature(entry, lineText, commandContext, position.character);
        const arg = getArgAtCursor(entry, argIndex);
        const argToken = extractArgTokenAtCursor(
          lineText,
          commandContext,
          position.character,
          entry.signature?.joiner ?? ","
        );
        if (arg) {
          markdown.appendMarkdown(`\n**Parameter at cursor:** ${formatArgDetail(arg)}\n\n`);
          if (argToken?.text) {
            markdown.appendMarkdown(`**Value at cursor:** \`${escapeMarkdown(argToken.text)}\`\n\n`);
          }
        }
        if (entry.args && entry.args.length > 0) {
          const argsSummary = entry.args
            .map((argDoc) => formatArgSummary(argDoc))
            .join("\n");
          markdown.appendMarkdown(`\nArguments:\n${argsSummary}\n`);
        }
        appendHoverOnlyNotes(markdown, document, position, hoverNoteIssues, lineIndexCache);

        if (argToken) {
          const hoverRange = new vscode.Range(
            position.line,
            argToken.start,
            position.line,
            argToken.end
          );
          return new vscode.Hover(markdown, hoverRange);
        }
        return new vscode.Hover(
          markdown,
          new vscode.Range(position.line, commandContext.commandStart, position.line, commandContext.commandEnd)
        );
      },
    })
  );

  context.subscriptions.push(
    vscode.languages.registerCompletionItemProvider(
      "zpl",
      {
        provideCompletionItems: async (document, position, token) => {
          if (token.isCancellationRequested) {
            return [];
          }
          docsBundlePromise ??= initializeDocsBundle(context);
          const docsBundle = await docsBundlePromise;
          if (token.isCancellationRequested) {
            return [];
          }

          const lineText = document.lineAt(position.line).text;
          const beforeCursor = lineText.slice(0, position.character);
          const commandPrefixMatch = beforeCursor.match(COMMAND_PREFIX_REGEX);
          if (commandPrefixMatch) {
            const prefix = commandPrefixMatch[0].toUpperCase().slice(1);
            const range = new vscode.Range(
              position.translate(0, -commandPrefixMatch[0].length),
              position
            );
            const completionState = analyzeCompletionContext(document, position);
            return buildCommandCompletionItems(docsBundle, prefix, range, completionState);
          }

          const contextAtCursor = extractCommandContextAtPosition(document, position);
          if (!contextAtCursor) {
            return [];
          }
          const entry = resolveCommandDoc(docsBundle, contextAtCursor.code);
          const argIndex = resolveArgIndexWithSignature(
            entry,
            lineText,
            contextAtCursor,
            position.character
          );
          const arg = getArgAtCursor(entry, argIndex);
          if (!arg?.enum || arg.enum.length === 0) {
            return [];
          }

          const currentArgText = getCurrentArgTokenText(
            lineText,
            contextAtCursor,
            position.character,
            entry?.signature?.joiner ?? ","
          );
          return buildEnumCompletionItems(arg, currentArgText, position);
        },
      },
      ...COMMAND_COMPLETION_TRIGGER_CHARACTERS
    )
  );

  const codeActionProvider: vscode.CodeActionProvider = {
    provideCodeActions: (
      _document,
      _range,
      context,
      _token
    ): vscode.ProviderResult<vscode.Command[]> => {
      const actions: vscode.Command[] = [];
      for (const diagnostic of context.diagnostics) {
        const id = typeof diagnostic.code === "string" ? diagnostic.code : undefined;
        if (!id) {
          continue;
        }
        actions.push({
          title: `Explain ${id}`,
          command: EXPLAIN_COMMAND_ID,
          tooltip: "Explain this diagnostic code",
          arguments: [id],
        });
      }
      return actions;
    },
  };

  context.subscriptions.push(
    vscode.languages.registerCodeActionsProvider("zpl", codeActionProvider, {
      providedCodeActionKinds: [vscode.CodeActionKind.QuickFix],
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(EXPLAIN_COMMAND_ID, async (diagnosticId?: string) => {
      if (!diagnosticId) {
        vscode.window.showWarningMessage("No diagnostic id provided.");
        return;
      }

      const core = await getCoreApi();
      const explanation = core.explain(diagnosticId);
      if (!explanation) {
        vscode.window.showInformationMessage(
          `${diagnosticId}: No explanation found in toolchain metadata.`
        );
        return;
      }

      const openDocs = "Open Diagnostic Docs";
      const choice = await vscode.window.showInformationMessage(
        `${diagnosticId}: ${explanation}`,
        openDocs
      );
      if (choice === openDocs) {
        await vscode.env.openExternal(
          vscode.Uri.parse(
            "https://github.com/trevordcampbell/zpl-toolchain/blob/main/docs/DIAGNOSTIC_CODES.md"
          )
        );
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      APPLY_THEME_PRESET_COMMAND_ID,
      async (presetOverride?: ThemePreset) => {
        const preset =
          presetOverride ??
          (await promptThemePresetSelection()) ??
          getThemePreset();
        const target = vscode.workspace.workspaceFolders?.length
          ? vscode.ConfigurationTarget.Workspace
          : vscode.ConfigurationTarget.Global;
        await getConfig().update("themePreset", preset, target);
        await applyThemePreset(preset, "manual");
      }
    )
  );

  if (getThemePreset() !== "custom") {
    await applyThemePreset(getThemePreset(), "auto");
  }

  // No-op in MVP, but keeps a stable integration seam for renderer preview.
  void rendererBridge;
}

export function deactivate(): void {
  // No explicit teardown required; disposables are registered on extension context.
}

async function initializeDocsBundle(context: vscode.ExtensionContext): Promise<DocsBundle> {
  const docsPath = context.asAbsolutePath(path.join("resources", "docs_bundle.json"));
  try {
    return await loadDocsBundle(docsPath);
  } catch (error) {
    console.warn(`zpl-toolchain: failed to load docs bundle: ${String(error)}`);
    return { by_code: {} };
  }
}

function toVsDiagnostics(
  document: vscode.TextDocument,
  issues: CoreDiagnostic[],
  lineIndexCache: Map<string, { version: number; index: Utf8LineIndex }>
): vscode.Diagnostic[] {
  const index = getOrCreateUtf8Index(document, lineIndexCache);
  return issues.map((issue) => {
    const range = issue.span
      ? new vscode.Range(
          index.positionAtByteOffset(issue.span.start),
          index.positionAtByteOffset(issue.span.end)
        )
      : new vscode.Range(new vscode.Position(0, 0), new vscode.Position(0, 0));

    const diagnostic = new vscode.Diagnostic(range, issue.message, mapSeverity(issue.severity));
    diagnostic.source = DIAGNOSTIC_SOURCE;
    diagnostic.code = issue.id;
    return diagnostic;
  });
}

function getOrCreateUtf8Index(
  document: vscode.TextDocument,
  lineIndexCache: Map<string, { version: number; index: Utf8LineIndex }>
): Utf8LineIndex {
  const cacheKey = document.uri.toString();
  const cached = lineIndexCache.get(cacheKey);
  const index =
    cached && cached.version === document.version
      ? cached.index
      : new Utf8LineIndex(document.getText());
  if (!cached || cached.version !== document.version) {
    lineIndexCache.set(cacheKey, { version: document.version, index });
  }
  return index;
}

function appendHoverOnlyNotes(
  markdown: vscode.MarkdownString,
  document: vscode.TextDocument,
  position: vscode.Position,
  hoverNoteIssues: Map<string, { version: number; issues: CoreDiagnostic[] }>,
  lineIndexCache: Map<string, { version: number; index: Utf8LineIndex }>
): void {
  const cachedNotes = hoverNoteIssues.get(document.uri.toString());
  if (!cachedNotes || cachedNotes.version !== document.version || cachedNotes.issues.length === 0) {
    return;
  }
  const notes = cachedNotes.issues;
  const byteOffset = Buffer.byteLength(
    document.getText(new vscode.Range(new vscode.Position(0, 0), position)),
    "utf8"
  );
  const matching = notes.filter((note) => {
    const span = note.span;
    if (!span) {
      return false;
    }
    return byteOffset >= span.start && byteOffset <= span.end;
  });
  if (matching.length === 0) {
    return;
  }
  markdown.appendMarkdown("\n---\n\n**Additional Notes**\n");
  for (const note of matching) {
    markdown.appendMarkdown(`- ${escapeMarkdown(note.message)}\n`);
  }
}

function mapSeverity(severity: CoreDiagnostic["severity"]): vscode.DiagnosticSeverity {
  switch (severity) {
    case "error":
      return vscode.DiagnosticSeverity.Error;
    case "warn":
      return vscode.DiagnosticSeverity.Warning;
    case "info":
      return vscode.DiagnosticSeverity.Information;
    default:
      return vscode.DiagnosticSeverity.Warning;
  }
}

function escapeMarkdown(value: string): string {
  return value.replace(/([\\`*_{}[\]()#+\-.!|>])/g, "\\$1");
}

function formatArgSummary(arg: DocsArg): string {
  const key = arg.key ?? "?";
  const name = arg.name ?? "arg";
  const type = arg.type ?? "unknown";
  const optional = arg.optional ? "optional" : "required";
  const unit = arg.unit ? `, unit: ${arg.unit}` : "";
  const range =
    Array.isArray(arg.range) && arg.range.length === 2
      ? `, range: ${arg.range[0]}-${arg.range[1]}`
      : "";
  const enumValues =
    Array.isArray(arg.enum) && arg.enum.length > 0
      ? `, values: ${arg.enum.map((v) => `\`${escapeMarkdown(String(v))}\``).join(", ")}`
      : "";
  return `- \`${key}\` ${escapeMarkdown(name)} (${escapeMarkdown(type)}, ${optional}${unit}${range}${enumValues})`;
}

function formatArgDetail(arg: DocsArg): string {
  const key = arg.key ?? "?";
  const name = arg.name ?? "arg";
  const type = arg.type ?? "unknown";
  const optional = arg.optional ? "optional" : "required";
  const details: string[] = [
    `\`${key}\` ${escapeMarkdown(name)} (${escapeMarkdown(type)}, ${optional})`,
  ];
  if (arg.unit) {
    details.push(`unit: ${escapeMarkdown(arg.unit)}`);
  }
  if (Array.isArray(arg.range) && arg.range.length === 2) {
    details.push(`range: ${arg.range[0]}-${arg.range[1]}`);
  }
  if (Array.isArray(arg.enum) && arg.enum.length > 0) {
    details.push(
      `values: ${arg.enum.map((value) => `\`${escapeMarkdown(String(value))}\``).join(", ")}`
    );
  }
  return details.join(" · ");
}

function getArgAtCursor(entry: CommandDoc | undefined, argIndex: number | null) {
  if (!entry || argIndex === null || !entry.args || argIndex < 0 || argIndex >= entry.args.length) {
    return undefined;
  }
  return entry.args[argIndex];
}

function buildCommandCompletionItems(
  docsBundle: DocsBundle,
  prefixWithoutLeader: string,
  replaceRange: vscode.Range,
  completionContext: CompletionContextState
): vscode.CompletionItem[] {
  const normalizedPrefix = prefixWithoutLeader.toUpperCase();
  return Object.entries(docsBundle.by_code)
    .filter(([opcode]) => opcode.slice(1).startsWith(normalizedPrefix))
    .sort(([leftOpcode, leftSource], [rightOpcode, rightSource]) => {
      const leftResolved = resolveCommandDoc(docsBundle, leftOpcode) ?? leftSource;
      const rightResolved = resolveCommandDoc(docsBundle, rightOpcode) ?? rightSource;
      const leftScopeRank = scopeRankForCompletion(leftResolved.scope, completionContext);
      const rightScopeRank = scopeRankForCompletion(rightResolved.scope, completionContext);
      if (leftScopeRank !== rightScopeRank) {
        return leftScopeRank - rightScopeRank;
      }
      const leftAliasRank = leftSource.aliasOf ? 1 : 0;
      const rightAliasRank = rightSource.aliasOf ? 1 : 0;
      if (leftAliasRank !== rightAliasRank) {
        return leftAliasRank - rightAliasRank;
      }
      return leftOpcode.localeCompare(rightOpcode);
    })
    .map(([opcode, sourceEntry]) => {
      const resolvedEntry = resolveCommandDoc(docsBundle, opcode) ?? sourceEntry;
      const item = new vscode.CompletionItem(
        buildCompletionRowLabel(opcode, sourceEntry, resolvedEntry),
        completionKindForCategory(resolvedEntry.category)
      );
      item.range = replaceRange;
      item.detail = buildCommandCompletionListDetail(opcode, sourceEntry, resolvedEntry);
      item.documentation = buildCommandCompletionDocumentation(opcode, sourceEntry, resolvedEntry);
      // Keep filtering aligned with the typed token (for example "^F" or "~H").
      // Omitting the command leader here causes VS Code to hide all command completions.
      item.filterText = opcode;
      const snippetText = buildCommandSnippetText(resolvedEntry);
      if (snippetText) {
        item.insertText = new vscode.SnippetString(snippetText);
      } else {
        item.insertText = opcode;
      }
      item.commitCharacters = [",", "^", "~"];
      const sortScope = scopeRankForCompletion(resolvedEntry.scope, completionContext)
        .toString()
        .padStart(2, "0");
      const sortAlias = sourceEntry.aliasOf ? "1" : "0";
      item.sortText = `${sortScope}_${sortAlias}_${opcode}`;
      return item;
    });
}

function resolveCommandDoc(docsBundle: DocsBundle, code: string): CommandDoc | undefined {
  const direct = docsBundle.by_code[code];
  if (direct && direct.hasSpec !== false) {
    return direct;
  }

  // ^A0-^A9 are compact aliases of ^A with font-selection first argument in-line.
  if (!direct && /^\^A[0-9]$/.test(code)) {
    const baseA = docsBundle.by_code["^A"];
    if (baseA && baseA.hasSpec !== false) {
      return baseA;
    }
  }

  if (direct?.aliasOf) {
    const aliasTarget = docsBundle.by_code[direct.aliasOf];
    if (aliasTarget && aliasTarget.hasSpec !== false) {
      return aliasTarget;
    }
  }

  if (direct?.anchor) {
    const anchored = Object.values(docsBundle.by_code).find(
      (candidate) => candidate.anchor === direct.anchor && candidate.hasSpec !== false
    );
    if (anchored) {
      return anchored;
    }
  }

  return direct;
}

function buildEnumCompletionItems(
  arg: DocsArg,
  currentArgText: string,
  position: vscode.Position
): vscode.CompletionItem[] {
  const values = Array.isArray(arg.enum) ? arg.enum : [];
  const normalizedToken = currentArgText.trim().toUpperCase();
  const filteredValues = normalizedToken
    ? values.filter((value) => String(value).toUpperCase().startsWith(normalizedToken))
    : values;
  const replaceRange = new vscode.Range(
    position.translate(0, -currentArgText.length),
    position
  );

  return filteredValues.map((value) => {
    const text = String(value);
    const item = new vscode.CompletionItem(text, vscode.CompletionItemKind.EnumMember);
    item.range = replaceRange;
    item.detail = formatArgDetail(arg);
    item.insertText = text;
    item.sortText = `1_${text}`;
    item.commitCharacters = [","];
    return item;
  });
}

function buildCommandSnippetText(entry: CommandDoc): string | undefined {
  if (!entry.formatTemplate) {
    return undefined;
  }
  let tabIndex = 1;
  return entry.formatTemplate.replace(/\{([^}]+)\}/g, (_match, rawKey: string) => {
    const key = String(rawKey).trim();
    return `\${${tabIndex++}:${key}}`;
  });
}

function buildCommandCompletionListDetail(
  opcode: string,
  sourceEntry: CommandDoc,
  resolvedEntry: CommandDoc
): string {
  const summary = summarizeForCompletionList(resolvedEntry.docs) ?? "ZPL command";
  if (sourceEntry.aliasOf) {
    return `Alias of ${sourceEntry.aliasOf} - ${summary}`;
  }
  return summary;
}

function summarizeForCompletionList(text: string | undefined, maxLen = 160): string | undefined {
  if (!text) {
    return undefined;
  }
  const oneLine = text.replace(/\s+/g, " ").trim();
  if (oneLine.length === 0) {
    return undefined;
  }
  if (oneLine.length <= maxLen) {
    return oneLine;
  }
  return `${oneLine.slice(0, maxLen - 1).trimEnd()}...`;
}

function formatCommandBadges(entry: CommandDoc): string {
  const category = entry.category?.trim();
  const scope = entry.scope?.trim();
  const badges = [category, scope]
    .filter((value): value is string => Boolean(value))
    .map((value) => value.toUpperCase());
  return badges.length > 0 ? `[${badges.join(" | ")}]` : "";
}

function formatCompletionName(sourceEntry: CommandDoc, resolvedEntry: CommandDoc): string {
  const base = resolvedEntry.name?.trim() || sourceEntry.name?.trim() || "";
  const badge = formatCompactBadge(resolvedEntry);
  if (!base) {
    const fallback = sourceEntry.aliasOf ? `Alias of ${sourceEntry.aliasOf}` : "";
    return badge ? `${fallback} ${badge}`.trim() : fallback;
  }
  const named = sourceEntry.aliasOf ? `${base} (alias)` : base;
  return badge ? `${named} ${badge}` : named;
}

function buildCompletionRowLabel(
  opcode: string,
  sourceEntry: CommandDoc,
  resolvedEntry: CommandDoc
): string {
  const rowText = formatCompletionName(sourceEntry, resolvedEntry);
  if (!rowText) {
    return opcode;
  }
  return `${opcode} ${truncateCompletionRowText(rowText)}`;
}

function truncateCompletionRowText(text: string, maxLen = 52): string {
  const compact = text.replace(/\s+/g, " ").trim();
  if (compact.length <= maxLen) {
    return compact;
  }
  return `${compact.slice(0, maxLen - 1).trimEnd()}...`;
}

function formatCompactBadge(entry: CommandDoc): string {
  const category = entry.category?.trim().toUpperCase();
  const scope = compactScope(entry.scope);
  if (category && scope) {
    return `[${category}/${scope}]`;
  }
  if (category) {
    return `[${category}]`;
  }
  if (scope) {
    return `[${scope}]`;
  }
  return "";
}

function compactScope(scope: string | undefined): string {
  switch (scope?.trim().toLowerCase()) {
    case "field":
      return "FLD";
    case "label":
      return "LBL";
    case "document":
      return "DOC";
    case "session":
      return "SES";
    case "job":
      return "JOB";
    default:
      return scope?.trim().toUpperCase() ?? "";
  }
}

function completionKindForCategory(category: string | undefined): vscode.CompletionItemKind {
  switch (category?.toLowerCase()) {
    case "text":
      return vscode.CompletionItemKind.Text;
    case "barcode":
      return vscode.CompletionItemKind.Struct;
    case "graphics":
      return vscode.CompletionItemKind.Color;
    case "format":
      return vscode.CompletionItemKind.Function;
    case "media":
      return vscode.CompletionItemKind.Constant;
    case "rfid":
      return vscode.CompletionItemKind.Event;
    case "network":
    case "wireless":
      return vscode.CompletionItemKind.Interface;
    case "storage":
      return vscode.CompletionItemKind.File;
    case "host":
    case "device":
    case "config":
    case "kdu":
    case "misc":
    default:
      return vscode.CompletionItemKind.Module;
  }
}

function buildCommandCompletionDocumentation(
  opcode: string,
  sourceEntry: CommandDoc,
  entry: CommandDoc
): vscode.MarkdownString {
  const markdown = new vscode.MarkdownString();
  const name = entry.name?.trim();
  markdown.appendMarkdown(name ? `### ${opcode} - ${escapeMarkdown(name)}\n\n` : `### ${opcode}\n\n`);
  if (sourceEntry.aliasOf) {
    markdown.appendMarkdown(`_Alias of \`${sourceEntry.aliasOf}\`._\n\n`);
  }
  const badges = formatCommandBadges(entry);
  if (badges) {
    markdown.appendMarkdown(`**${badges}**\n\n`);
  }
  if (entry.docs) {
    markdown.appendMarkdown(`${entry.docs}\n\n`);
  }
  if (entry.formatTemplate) {
    markdown.appendCodeblock(entry.formatTemplate, "text");
  }
  if (entry.args && entry.args.length > 0) {
    markdown.appendMarkdown(
      `\nArguments:\n${entry.args.map((argDoc) => formatArgSummary(argDoc)).join("\n")}\n`
    );
  }
  markdown.isTrusted = false;
  return markdown;
}

function analyzeCompletionContext(
  document: vscode.TextDocument,
  position: vscode.Position
): CompletionContextState {
  const textBeforeCursor = document.getText(
    new vscode.Range(new vscode.Position(0, 0), position)
  );
  const commandRegex = new RegExp(COMMAND_TOKEN_REGEX_SOURCE, "gi");
  let inLabel = false;
  let inField = false;
  for (const match of textBeforeCursor.matchAll(commandRegex)) {
    const code = (match[0] ?? "").toUpperCase();
    if (code === "^XA") {
      inLabel = true;
      inField = false;
      continue;
    }
    if (code === "^XZ") {
      inLabel = false;
      inField = false;
      continue;
    }
    if (!inLabel) {
      continue;
    }
    if (code === "^FS") {
      inField = false;
      continue;
    }
    if (FIELD_OPENING_COMMANDS.has(code)) {
      inField = true;
    }
  }
  return { inLabel, inField };
}

function scopeRankForCompletion(
  scope: string | undefined,
  context: CompletionContextState
): number {
  switch (scope?.toLowerCase()) {
    case "field":
      return context.inField ? 0 : context.inLabel ? 1 : 4;
    case "label":
      return context.inField ? 1 : context.inLabel ? 0 : 3;
    case "document":
      return context.inLabel ? 3 : 0;
    case "session":
      return context.inLabel ? 4 : 1;
    case "job":
      return 2;
    default:
      return 5;
  }
}

function extractCommandContextAtPosition(
  document: vscode.TextDocument,
  position: vscode.Position
): CommandContext | undefined {
  const lineText = document.lineAt(position.line).text;
  const regex = new RegExp(COMMAND_TOKEN_REGEX_SOURCE, "g");
  const matches = [...lineText.matchAll(regex)];
  for (let i = 0; i < matches.length; i += 1) {
    const current = matches[i];
    const code = current[0].toUpperCase();
    const start = current.index ?? 0;
    const end = i + 1 < matches.length ? (matches[i + 1].index ?? lineText.length) : lineText.length;
    if (position.character < start || position.character >= end) {
      continue;
    }

    const relative = position.character - start;
    if (relative <= code.length) {
      return { code, argIndex: null, commandStart: start, commandEnd: end };
    }

    const commandSegment = lineText.slice(
      start + code.length,
      Math.max(start + code.length, position.character)
    );
    const argIndex = commandSegment.split(",").length - 1;
    return { code, argIndex, commandStart: start, commandEnd: end };
  }
  return undefined;
}

function getCurrentArgTokenText(
  lineText: string,
  context: CommandContext,
  cursorCharacter: number,
  joiner = ","
): string {
  if (context.argIndex === null) {
    return "";
  }
  const argsStart = context.commandStart + context.code.length;
  const beforeCursor = lineText.slice(argsStart, cursorCharacter);
  const tokens = beforeCursor.split(joiner);
  return tokens[tokens.length - 1] ?? "";
}

function extractArgTokenAtCursor(
  lineText: string,
  context: CommandContext,
  cursorCharacter: number,
  joiner = ","
): { text: string; start: number; end: number } | undefined {
  if (context.argIndex === null) {
    return undefined;
  }
  const argsStart = context.commandStart + context.code.length;
  const boundedCursor = Math.max(argsStart, Math.min(cursorCharacter, context.commandEnd));
  const beforeCursor = lineText.slice(argsStart, boundedCursor);
  const lastJoiner = beforeCursor.lastIndexOf(joiner);
  const tokenStartRel = lastJoiner >= 0 ? lastJoiner + joiner.length : 0;
  const tokenStart = argsStart + tokenStartRel;
  const tail = lineText.slice(tokenStart, context.commandEnd);
  const joinerOffset = tail.indexOf(joiner);
  const tokenEnd = joinerOffset >= 0 ? tokenStart + joinerOffset : context.commandEnd;
  const text = lineText.slice(tokenStart, tokenEnd).trim();
  return { text, start: tokenStart, end: tokenEnd };
}

function isZplDocument(document: vscode.TextDocument): boolean {
  return document.languageId === "zpl";
}

function getConfig(): vscode.WorkspaceConfiguration {
  return vscode.workspace.getConfiguration("zplToolchain");
}

function getDebounceMs(): number {
  const value = getConfig().get<number>("diagnostics.debounceMs", 150);
  return Number.isFinite(value) ? Math.max(0, Math.min(value, 2000)) : 150;
}

function getProfileJson(): string {
  return getConfig().get<string>("profileJson", "");
}

function getIndentStyle(): "none" | "label" | "field" {
  const indent = getConfig().get<string>("format.indent", "none");
  return indent === "label" || indent === "field" ? indent : "none";
}

function getFormatCompaction(): FormatCompaction {
  const compaction = getConfig().get<string>("format.compaction", "none");
  return compaction === "field" ? "field" : "none";
}

function getCommentPlacement(): CommentPlacement {
  const placement = getConfig().get<string>("format.commentPlacement", "inline");
  return placement === "line" ? "line" : "inline";
}

function isHoverEnabled(): boolean {
  return getConfig().get<boolean>("hover.enabled", true);
}

function getThemePreset(): ThemePreset {
  const preset = getConfig().get<string>("themePreset", "custom");
  if (
    preset === "default" ||
    preset === "high-contrast" ||
    preset === "minimal" ||
    preset === "custom"
  ) {
    return preset;
  }
  return "custom";
}

function getThemePresetRules(
  preset: Exclude<ThemePreset, "custom">
): Array<{
  name: string;
  scope: string | string[];
  settings: { foreground?: string; fontStyle?: string };
}> {
  if (preset === "high-contrast") {
    return [
      {
        name: "zpl-toolchain:commands",
        scope: "keyword.control.command.zpl",
        settings: { foreground: "#FF7B72", fontStyle: "bold" },
      },
      {
        name: "zpl-toolchain:field-data",
        scope: "string.unquoted.field-data.zpl",
        settings: { foreground: "#9CDCFE" },
      },
      {
        name: "zpl-toolchain:numbers",
        scope: "constant.numeric.zpl",
        settings: { foreground: "#FFD580" },
      },
      {
        name: "zpl-toolchain:comments",
        scope: "comment.line.semicolon.zpl",
        settings: { foreground: "#8B949E", fontStyle: "italic" },
      },
    ];
  }
  if (preset === "minimal") {
    return [
      {
        name: "zpl-toolchain:commands",
        scope: "keyword.control.command.zpl",
        settings: { foreground: "#C586C0" },
      },
      {
        name: "zpl-toolchain:field-data",
        scope: "string.unquoted.field-data.zpl",
        settings: { foreground: "#D4D4D4" },
      },
      {
        name: "zpl-toolchain:numbers",
        scope: "constant.numeric.zpl",
        settings: { foreground: "#B5CEA8" },
      },
    ];
  }
  return [
    {
      name: "zpl-toolchain:commands",
      scope: "keyword.control.command.zpl",
      settings: { foreground: "#C586C0", fontStyle: "bold" },
    },
    {
      name: "zpl-toolchain:field-data",
      scope: "string.unquoted.field-data.zpl",
      settings: { foreground: "#9CDCFE" },
    },
    {
      name: "zpl-toolchain:punctuation",
      scope: "punctuation.separator.parameter.zpl",
      settings: { foreground: "#D4D4D4" },
    },
    {
      name: "zpl-toolchain:numbers",
      scope: "constant.numeric.zpl",
      settings: { foreground: "#B5CEA8" },
    },
    {
      name: "zpl-toolchain:comments",
      scope: "comment.line.semicolon.zpl",
      settings: { foreground: "#6A9955", fontStyle: "italic" },
    },
  ];
}

async function promptThemePresetSelection(): Promise<ThemePreset | undefined> {
  const selection = await vscode.window.showQuickPick(
    [
      { label: "Custom (remove extension preset rules)", value: "custom" as ThemePreset },
      { label: "Default", value: "default" as ThemePreset },
      { label: "High Contrast", value: "high-contrast" as ThemePreset },
      { label: "Minimal", value: "minimal" as ThemePreset },
    ],
    {
      title: "Choose ZPL Toolchain Theme Preset",
      placeHolder: "Apply token color rules for ZPL scopes",
    }
  );
  return selection?.value;
}

async function applyThemePreset(
  preset: ThemePreset,
  source: "auto" | "manual"
): Promise<void> {
  const configuration = vscode.workspace.getConfiguration();
  const current = configuration.get<Record<string, unknown>>("editor.tokenColorCustomizations");
  const textMateRulesCurrent = Array.isArray(current?.textMateRules)
    ? [...(current?.textMateRules as unknown[])]
    : [];
  const textMateRules = textMateRulesCurrent.filter((rule) => {
    if (!rule || typeof rule !== "object") {
      return true;
    }
    const name = (rule as { name?: unknown }).name;
    return typeof name !== "string" || !name.startsWith("zpl-toolchain:");
  });

  if (preset !== "custom") {
    textMateRules.push(...getThemePresetRules(preset));
  }

  const next = {
    ...(current ?? {}),
    textMateRules,
  };
  if (JSON.stringify(current ?? {}) === JSON.stringify(next)) {
    return;
  }

  const target = vscode.workspace.workspaceFolders?.length
    ? vscode.ConfigurationTarget.Workspace
    : vscode.ConfigurationTarget.Global;
  await configuration.update("editor.tokenColorCustomizations", next, target);

  if (source === "manual") {
    vscode.window.showInformationMessage(
      preset === "custom"
        ? "ZPL Toolchain: removed extension preset token colors."
        : `ZPL Toolchain: applied '${preset}' token color preset.`
    );
  }
}
