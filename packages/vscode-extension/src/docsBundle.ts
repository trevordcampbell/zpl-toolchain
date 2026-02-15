import fs from "node:fs/promises";

export interface DocsArg {
  key?: string | null;
  name?: string | null;
  type?: string | null;
  optional?: boolean | null;
  unit?: string | null;
  default?: string | number | boolean | null;
  enum?: Array<string | number | boolean> | null;
  range?: [number, number] | null;
}

export interface DocsSplitRule {
  paramIndex?: number | null;
  charCounts?: number[] | null;
}

export interface DocsSignature {
  params?: string[] | null;
  joiner?: string | null;
  splitRule?: DocsSplitRule | null;
}

export interface CommandDoc {
  anchor?: string;
  aliasOf?: string;
  hasSpec?: boolean;
  name?: string;
  category?: string;
  scope?: string;
  docs?: string;
  formatTemplate?: string;
  signature?: DocsSignature;
  args?: DocsArg[];
}

export interface DocsBundle {
  all_codes?: string[];
  by_code: Record<string, CommandDoc>;
}

export async function loadDocsBundle(path: string): Promise<DocsBundle> {
  const content = await fs.readFile(path, "utf8");
  return JSON.parse(content) as DocsBundle;
}
