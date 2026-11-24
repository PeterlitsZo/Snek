English | [简体中文](./docs/index.zh_CN.md) |
[繁體中文](./docs/index.zh_TW.md) | [日本語](./docs/index.ja_JP.md) |
[한국어](./docs/index.ko_KR.md) | [Español](./docs/index.es_ES.md) |
[Français](./docs/index.fr_FR.md) | [Deutsch](./docs/index.de_DE.md) |
[Português (Brasil)](./docs/index.pt_BR.md)

# Snek

Snek is a collection of practical tools that provides a TypeScript runtime
interface.

Consider the following script:

```ts
import { $ } from "snek";

let output = await $`echo "Hello World"`.text();
console.log(JSON.stringify(output));
```

The script runs the `echo "Hello World"` command, captures its standard output,
and stores it in the `output` variable. You can then use `JSON`, `console`, and
other tools to process it further.

## Reference

### Running commands

Run a command, or use `.text()` to get its output:

```ts
import { $ } from "snek";

await $`echo "Hello World"`;

let name = "Snek";
let output = await $`echo ${name}`.text();
console.log(output); // "Snek\n"
```

If a command outputs JSON, parse it directly:

```ts
import { $ } from "snek";

let data = await $`echo '{"name":"Snek"}'`.json<{ name: string }>();
console.log(data.name); // "Snek"
```

Read the output line by line:

```ts
import { $ } from "snek";

for await (let line of $`printf 'first\nsecond\n'`.lines()) {
  console.log(line); // "first", then "second"
}
```

### Working with binary data

You can modify a `Buffer` byte by byte. A `Blob` keeps the data from when it was
created, even if the original buffer changes:

```ts
import { Buffer, Blob } from "snek";

let bytes = Buffer.from("Hello");
let blob = new Blob([bytes], { type: "text/plain" });

bytes[0] = 104;

console.log(await new Blob([bytes]).text()); // "hello"
console.log(await blob.text()); // "Hello"
console.log(blob.size); // 5
console.log(blob.type); // "text/plain"
console.log(await blob.slice(0, 2).text()); // "He"
```

### Accessing S3

Enter your credentials and bucket name to read and write objects:

```ts
import { S3Client } from "snek";

let s3 = new S3Client({
  accessKeyId: "YOUR_ACCESS_KEY_ID",
  secretAccessKey: "YOUR_SECRET_ACCESS_KEY",
  bucket: "my-bucket",
  region: "us-east-1",
});

let file = s3.file("hello.txt");
await file.write("Hello World", { type: "text/plain" });
console.log(await file.text()); // "Hello World"

let url = s3.presign("hello.txt", {
  method: "GET",
  expiresIn: 3600,
});
console.log(url);
```

The download link expires in one hour. Change `method` to `"PUT"` to generate an
upload link.

To connect to an S3-compatible service, add `endpoint` to the client
configuration. For example, for Cloudflare R2:

```ts
import { S3Client } from "snek";

let s3 = new S3Client({
  accessKeyId: "YOUR_ACCESS_KEY_ID",
  secretAccessKey: "YOUR_SECRET_ACCESS_KEY",
  bucket: "my-bucket",
  region: "auto",
  endpoint: "https://YOUR_ACCOUNT_ID.r2.cloudflarestorage.com",
});

console.log(await s3.file("hello.txt").text());
```

### Command-line arguments

You can access command-line arguments through `argv`.

```ts
import { argv } from "snek";

console.log(argv); // [ SNEK_PATH, SCRIPT_PATH, "--hello", "world" ]
```

We provide `Cli` to parse command-line arguments. Its API is inspired by
[cac](https://github.com/cacjs/cac). Here is how to use it:

```ts
import { Cli } from "snek";

const cli = new Cli();
cli.option('--type [type]', 'Choose a project type', {
  default: 'node',
});
cli.option('--name <name>', 'Provide your name');

// Display help message when `-h` or `--help` appears.
cli.help();

// Display version number when `-v` or `--version` appears.
cli.version('0.0.0');

const parsed = cli.parse();

console.log(parsed);
```

You can then interact with the script like this.

```sh
snek app.ts --type web --name Alice
snek app.ts --help
snek app.ts --version
snek app.ts -- --name
```

