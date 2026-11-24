# Snek

Snek 是一系列實用的工具集，並提供了 TypeScript 執行階段介面。

以下列腳本為例：

```ts
import { $ } from "snek";

let output = await $`echo "Hello World"`.text();
console.log(JSON.stringify(output));
```

它會執行 `echo "Hello World"` 命令，擷取標準輸出並存入變數 `output` 中。接著，你
可以使用 `JSON`、`console` 等進一步處理。

## 參考

### 執行命令

直接執行命令，或用 `.text()` 取得輸出：

```ts
import { $ } from "snek";

await $`echo "Hello World"`;

let name = "Snek";
let output = await $`echo ${name}`.text();
console.log(output); // "Snek\n"
```

命令輸出 JSON 時，可以直接解析：

```ts
import { $ } from "snek";

let data = await $`echo '{"name":"Snek"}'`.json<{ name: string }>();
console.log(data.name); // "Snek"
```

逐行讀取輸出：

```ts
import { $ } from "snek";

for await (let line of $`printf 'first\nsecond\n'`.lines()) {
  console.log(line); // "first", then "second"
}
```

### 處理二進位資料

`Buffer` 可以逐位元組修改；`Blob` 保留建立時的資料，不隨原始 buffer 改變：

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

### 存取 S3

填入自己的憑證和儲存桶名稱，就可以讀寫物件：

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

這裡產生的下載連結一小時後到期。將 `method` 改成 `"PUT"` 可產生上傳連結。

連接相容 S3 的服務時，在用戶端設定中加入 `endpoint`。例如 Cloudflare R2：

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

### 命令列參數

你可以透過 `argv` 取得命令列參數。

```ts
import { argv } from "snek";

console.log(argv); // [ SNEK_PATH, SCRIPT_PATH, "--hello", "world" ]
```

我們提供 `Cli` 用於解析命令列參數。它的 API 參考
[cac](https://github.com/cacjs/cac)，你可以這樣使用它：

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

接著我們可以這樣與它互動。

```sh
snek app.ts --type web --name Alice
snek app.ts --help
snek app.ts --version
snek app.ts -- --name
```
