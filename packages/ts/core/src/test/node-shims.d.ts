declare module "node:test" {
  export function describe(name: string, fn: () => void): void;
  export function it(name: string, fn: () => void | Promise<void>): void;
}

declare module "node:assert/strict" {
  const assert: {
    throws(fn: () => unknown, error?: RegExp): void;
    ok(value: unknown, message?: string): void;
  };
  export default assert;
}
