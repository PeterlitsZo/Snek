# Snek

Snek 是一系列实用的工具集，并提供了 TypeScript 运行时接口。

对于如下的脚本而言：

```ts
import { $ } from "snek";

let output = await $`echo "Hello World"`.text();
console.log(JSON.stringify(output));
```

它会执行 `echo "Hello World"` 命令，并将标准输出捕获，并放入变量 `output` 中，最
后你可以使用 `JSON`、`console` 等进一步操作。

## 安装

执行如下的命令完成安装：

```bash
curl -fsSL https://peterlits.net/snek/install.sh | bash
```

默认情况下，它会安装到 `~/.local/bin/` 下。

## 参考

### 执行命令

直接执行命令，或用 `.text()` 拿到输出：

```ts
import { $ } from "snek";

await $`echo "Hello World"`;

let name = "Snek";
let output = await $`echo ${name}`.text();
console.log(output); // "Snek\n"
```

命令输出 JSON 时，可以直接解析：

```ts
import { $ } from "snek";

let data = await $`echo '{"name":"Snek"}'`.json<{ name: string }>();
console.log(data.name); // "Snek"
```

逐行读取输出：

```ts
import { $ } from "snek";

for await (let line of $`printf 'first\nsecond\n'`.lines()) {
  console.log(line); // "first", then "second"
}
```

### 处理二进制数据

`Buffer` 可以按字节修改；`Blob` 保存创建时的数据，不随原始 buffer 改变：

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

### 访问 S3

填入自己的凭据和存储桶名称，就可以读写对象：

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

这里生成的下载链接一小时后过期。将 `method` 改成 `"PUT"` 可生成上传链接。

连接兼容 S3 的服务时，在客户端配置中添加 `endpoint`。例如 Cloudflare R2：

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

### 命令行参数

你可以通过 `argv` 来获取命令行参数。

```ts
import { argv } from "snek";

console.log(argv); // [ SNEK_PATH, SCRIPT_PATH, "--hello", "world" ]
```

我们提供 `Cli` 用于解析命令行参数。它的 API 参考
[cac](https://github.com/cacjs/cac)，你可以这样使用它：

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

之后我们可以这样和它交互。

```sh
snek app.ts --type web --name Alice
snek app.ts --help
snek app.ts --version
snek app.ts -- --name
```
