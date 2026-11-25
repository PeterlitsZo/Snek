export interface ShellOutput {
  stdout: string;
  stderr: string;
  exitCode: number | null;
}

/**
 * A lazy command that runs once when awaited or consumed.
 *
 * Repeated waits share the result; execution errors reject the promise.
 */
export interface ShellCommand extends PromiseLike<void> {
  /** Capture both streams; nonzero exits resolve, execution errors reject. */
  nothrow(): Promise<ShellOutput>;

  /**
   * Merge environment overrides for this command, preserving inherited variables.
   * Snapshots all own properties; keys and values must be strings.
   * Throws TypeError for invalid input or after the command is consumed.
   */
  env(values: Record<string, string>): this;

  /**
   * Handle completion or failure, returning a Promise for the callback result.
   */
  then<TResult1 = void, TResult2 = never>(
    onfulfilled?: ((value: void) => TResult1 | PromiseLike<TResult1>) | null,
    onrejected?: ((reason: any) => TResult2 | PromiseLike<TResult2>) | null,
  ): Promise<TResult1 | TResult2>;

  /** Handle execution failure, optionally recovering with a new value. */
  catch<TResult = never>(
    onrejected?: ((reason: any) => TResult | PromiseLike<TResult>) | null,
  ): Promise<void | TResult>;

  /**
   * Run after success or failure; preserve the outcome unless the callback
   * fails.
   */
  finally(onfinally?: (() => void) | null): Promise<void>;

  /** Capture stdout as UTF-8 text, preserving trailing newlines. */
  text(): Promise<string>;

  /**
   * Capture stdout and parse it as JSON; invalid JSON rejects with SyntaxError.
   */
  json<T = unknown>(): Promise<T>;

  /** Capture stdout as immutable bytes with an empty MIME type. */
  blob(): Promise<Blob>;

  /**
   * Stream stdout lines; starts on first consumption, and early exit cancels
   * the process.
   */
  lines(): AsyncIterableIterator<string>;
}

/** Create a command, inserting interpolated values as literal arguments. */
export declare function $(
  strings: TemplateStringsArray,
  ...values: unknown[]
): ShellCommand;

/**
 * Minimal immutable Blob; accepts an array of parts and snapshots mutable
 * buffers.
 */
export declare class Blob {
  constructor(parts?: readonly (string | ArrayBuffer | Uint8Array<ArrayBuffer> | Blob)[], options?: { type?: string });
  readonly size: number;
  readonly type: string;
  text(): Promise<string>;
  arrayBuffer(): Promise<ArrayBuffer>;
  slice(start?: number, end?: number, contentType?: string): Blob;
}

/** Minimal UTF-8 Buffer, backed by Uint8Array. */
export interface Buffer extends Uint8Array<ArrayBuffer> {}
export declare const Buffer: {
  new (length: number): Buffer;
  new (buffer: ArrayBuffer, byteOffset?: number, length?: number): Buffer;
  new (values: ArrayLike<number>): Buffer;
  readonly prototype: Buffer;
  /** Strings use UTF-8; Uint8Array inputs are copied; ArrayBuffer inputs share memory. */
  from(value: string | ArrayBuffer | Uint8Array<ArrayBuffer>): Buffer;
};

export interface S3RequestOptions {
  /** Positive safe integer milliseconds; defaults to the client timeout. */
  timeoutMs?: number;
}

export interface S3ClientOptions extends S3RequestOptions {
  accessKeyId: string;
  secretAccessKey: string;
  bucket: string;
  /** Defaults to "us-east-1". Set "auto" explicitly for Cloudflare R2. */
  region?: string;
  /** HTTP(S) origin, without a path, query, fragment, or embedded credentials. */
  endpoint?: string;
  sessionToken?: string;
}

export interface S3PresignOptions {
  /** Defaults to "GET". */
  method?: "GET" | "PUT" | "HEAD" | "DELETE";
  /** Integer seconds from 1 to 604800. Defaults to 86400 (24 hours). */
  expiresIn?: number;
}

export interface S3WriteOptions extends S3RequestOptions {
  ifNoneMatch?: "*";
  metadata?: Record<string, string>;
  contentEncoding?: string;
  contentDisposition?: string;
  cacheControl?: string;
  /** Stored Content-Type; defaults to "application/octet-stream". */
  type?: string;
}

export interface S3Stat {
  size: number;
  type: string;
  etag: string;
  lastModified: Date;
  metadata: Record<string, string>;
  contentEncoding?: string;
  contentDisposition?: string;
  cacheControl?: string;
}

export type S3WriteData = string | Uint8Array<ArrayBuffer> | ArrayBuffer | Blob;

export interface S3File {
  text(options?: S3RequestOptions): Promise<string>;
  json<T = unknown>(options?: S3RequestOptions): Promise<T>;
  arrayBuffer(options?: S3RequestOptions): Promise<ArrayBuffer>;
  bytes(options?: S3RequestOptions): Promise<Uint8Array<ArrayBuffer>>;
  blob(options?: S3RequestOptions): Promise<Blob>;
  /** One PUT; inputs are snapshotted. Conditional writes are never retried automatically. */
  write(data: S3WriteData, options?: S3WriteOptions): Promise<number>;
  stat(options?: S3RequestOptions): Promise<S3Stat>;
  /** Only HTTP 404 becomes false; other failures reject. */
  exists(options?: S3RequestOptions): Promise<boolean>;
  delete(options?: S3RequestOptions): Promise<void>;
  presign(options?: S3PresignOptions): string;
}

/** Explicitly configured, path-style S3 client. No environment credential lookup. */
export declare class S3Client {
  constructor(options: S3ClientOptions);
  /** Creates a lazy reference without I/O; keys contain 1–1024 UTF-8 bytes. */
  file(key: string): S3File;
  write(key: string, data: S3WriteData, options?: S3WriteOptions): Promise<number>;
  stat(key: string, options?: S3RequestOptions): Promise<S3Stat>;
  exists(key: string, options?: S3RequestOptions): Promise<boolean>;
  delete(key: string, options?: S3RequestOptions): Promise<void>;
  /** Signs locally and synchronously, without I/O. */
  presign(key: string, options?: S3PresignOptions): string;
}

/** Read/write service and transport errors. Fields depend on the response. */
export interface S3Error extends Error {
  name: "S3Error";
  operation?: string;
  key?: string;
  status?: number;
  code?: string;
  requestId?: string;
}

/**
 * Process arguments: absolute executable path, resolved script path, then user
 * arguments.
 */
export declare const argv: string[];

export interface CliOptionConfig {
  default?: string | boolean;
}

export interface CliParseResult {
  args: string[];
  options: Record<string, string | boolean | undefined>;
}

export interface CliParseConfig {
  run?: boolean;
}

export interface CliCommand {
  option(flags: string, description: string, config?: CliOptionConfig): this;
  alias(name: string): this;
  action(callback: (...args: any[]) => any): this;
}

/** A small command-line parser with an API inspired by cac. */
export declare class Cli {
  constructor();
  command(declaration: string, description?: string): CliCommand;
  runMatchedCommand(): any;
  args: string[];
  options: Record<string, string | boolean | undefined>;
  option(flags: string, description: string, config?: CliOptionConfig): this;
  /**
   * Register `-h` / `--help`. Output happens during parse; the script
   * continues.
   */
  help(): this;
  /**
   * Register `-v` / `--version`. Output happens during parse; the script
   * continues.
   */
  version(value: string): this;
  /**
   * Parse script arguments.
   */
  parse(args?: string[], config?: CliParseConfig): CliParseResult;
}

declare global {
  interface ImportMeta {
    dir: string;
    file: string;
    path: string;
    url: string;
  }
}

/** Create a lazy local-file reference, resolving relative paths now. */
export declare function file(path: string): LocalFile;

export interface LocalFile {
  text(): Promise<string>;
  json<T = unknown>(): Promise<T>;
  arrayBuffer(): Promise<ArrayBuffer>;
  blob(): Promise<Blob>;
  /** Create or replace the file; return bytes written. Does not create parents. */
  write(data: string | Uint8Array<ArrayBuffer>): Promise<number>;
  /** Create or append; return bytes written. Does not create parents. */
  append(data: string | Uint8Array<ArrayBuffer>): Promise<number>;
  /** False for missing paths; directories and other filesystem errors reject. */
  exists(): Promise<boolean>;
}

export interface FileError extends Error {
  name: "FileError";
  operation: string;
  path: string;
  code: string;
}

export interface Process {
  readonly argv: string[];
  readonly execPath: string;
  readonly pid: number;
  readonly platform: string;
  readonly arch: string;
  readonly version: string;
  readonly env: Record<string, string | undefined>;
  cwd(): string;
  chdir(path: string): void;
  /** Integer from 0 to 255; applied after pending work completes. */
  exitCode: number;
}

export declare const process: Process;
