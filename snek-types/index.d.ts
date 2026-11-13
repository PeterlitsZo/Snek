/**
 * Execute a shell template through Rust, inheriting stdin, stdout, and stderr.
 * Currently supports one program and its arguments, without invoking an external shell.
 * Interpolated values are stringified and inserted as literal argument text.
 * Resolves when shell execution succeeds; rejects on invalid syntax or process failure.
 *
 * @example
 * await $`echo ${"Hello World!"}`;
 */
export declare function $(
  strings: TemplateStringsArray,
  ...values: unknown[]
): Promise<void>;
