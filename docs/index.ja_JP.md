# Snek

Snek は、TypeScript ランタイムインターフェースを備えた実用的なツール集です。

次のスクリプトを例に説明します。

```ts
import { $ } from "snek";

let output = await $`echo "Hello World"`.text();
console.log(JSON.stringify(output));
```

このスクリプトは `echo "Hello World"` コマンドを実行し、標準出力を変数 `output`
に格納します。その後、`JSON` や `console` などでさらに処理できます。

## インストール

次のコマンドを実行してインストールします。

```bash
curl -fsSL https://peterlits.net/snek/install.sh | bash
```

デフォルトでは、`~/.local/bin/` にインストールされます。

## リファレンス

### コマンドの実行

コマンドを実行するか、`.text()` で出力を取得します。

```ts
import { $ } from "snek";

await $`echo "Hello World"`;

let name = "Snek";
let output = await $`echo ${name}`.text();
console.log(output); // "Snek\n"
```

コマンドが JSON を出力する場合は、そのまま解析できます。

```ts
import { $ } from "snek";

let data = await $`echo '{"name":"Snek"}'`.json<{ name: string }>();
console.log(data.name); // "Snek"
```

出力を一行ずつ読み取ります。

```ts
import { $ } from "snek";

for await (let line of $`printf 'first\nsecond\n'`.lines()) {
  console.log(line); // "first", then "second"
}
```

### バイナリデータの操作

`Buffer` はバイト単位で変更できます。`Blob` は作成時のデータを保持し、元の
buffer を変更しても影響を受けません。

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

### S3 へのアクセス

自分の認証情報とバケット名を設定すると、オブジェクトを読み書きできます。

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

このダウンロードリンクは 1 時間後に期限切れになります。`method` を `"PUT"` にす
ると、アップロードリンクを生成できます。

S3 互換サービスに接続するには、クライアント設定に `endpoint` を追加します。
Cloudflare R2 の場合：

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

### コマンドライン引数

`argv` でコマンドライン引数を取得できます。

```ts
import { argv } from "snek";

console.log(argv); // [ SNEK_PATH, SCRIPT_PATH, "--hello", "world" ]
```

コマンドライン引数を解析するために `Cli` を提供しています。API は
[cac](https://github.com/cacjs/cac) を参考にしており、次のように使用できます。

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

次のように実行できます。

```sh
snek app.ts --type web --name Alice
snek app.ts --help
snek app.ts --version
snek app.ts -- --name
```
