/**
 * A lazy command that runs once when awaited or consumed.
 *
 * Repeated waits share the result; execution errors reject the promise.
 */
export interface ShellCommand extends PromiseLike<void> {
  catch<TResult = never>(
    onrejected?: ((reason: any) => TResult | PromiseLike<TResult>) | null,
  ): Promise<void | TResult>;
  finally(onfinally?: (() => void) | null): Promise<void>;
  /** Capture stdout as UTF-8 text, preserving trailing newlines. */
  text(): Promise<string>;
  /**
   * Capture stdout and parse it as JSON; invalid JSON rejects with SyntaxError.
   */
  json<T = unknown>(): Promise<T>;
  /** Capture stdout as immutable bytes with an empty MIME type. */
  blob(): Promise<Blob>;
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

export interface S3ClientOptions {
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
  method?: "GET" | "PUT";
  /** Integer seconds from 1 to 604800. Defaults to 86400 (24 hours). */
  expiresIn?: number;
}

export interface S3WriteOptions {
  /** Stored Content-Type; defaults to "application/octet-stream". */
  type?: string;
}

export interface S3File {
  /** Fresh GET, buffered in memory and decoded as UTF-8 with replacement. */
  text(): Promise<string>;
  /** Fresh GET parsed as JSON. Invalid JSON rejects with SyntaxError. */
  json<T = unknown>(): Promise<T>;
  /** Fresh GET, returning an independent ArrayBuffer. */
  arrayBuffer(): Promise<ArrayBuffer>;
  /** One PUT replacing the object. Snapshots input and resolves to its byte count. */
  write(data: string | Uint8Array<ArrayBuffer>, options?: S3WriteOptions): Promise<number>;
}

/** Explicitly configured, path-style S3 client. No environment credential lookup. */
export declare class S3Client {
  constructor(options: S3ClientOptions);
  /** Creates a lazy reference without I/O; keys contain 1–1024 UTF-8 bytes. */
  file(key: string): S3File;
  /** Signs locally and synchronously, without I/O. */
  presign(key: string, options?: S3PresignOptions): string;
}

/** Read/write service and transport errors. Fields depend on the response. */
export interface S3Error extends Error {
  name: "S3Error";
  status?: number;
  code?: string;
  requestId?: string;
}
