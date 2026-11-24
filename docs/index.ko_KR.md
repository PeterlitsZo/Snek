# Snek

Snek은 TypeScript 런타임 인터페이스를 제공하는 실용적인 도구 모음입니다.

다음 스크립트를 예로 들어 보겠습니다:

```ts
import { $ } from "snek";

let output = await $`echo "Hello World"`.text();
console.log(JSON.stringify(output));
```

이 스크립트는 `echo "Hello World"` 명령을 실행하고 표준 출력을 캡처하여 `output`
변수에 저장합니다. 이후 `JSON`, `console` 등을 사용해 추가로 처리할 수 있습니다.

## 레퍼런스

### 명령 실행

명령을 실행하거나 `.text()`로 출력을 가져옵니다:

```ts
import { $ } from "snek";

await $`echo "Hello World"`;

let name = "Snek";
let output = await $`echo ${name}`.text();
console.log(output); // "Snek\n"
```

명령이 JSON을 출력하면 바로 파싱할 수 있습니다:

```ts
import { $ } from "snek";

let data = await $`echo '{"name":"Snek"}'`.json<{ name: string }>();
console.log(data.name); // "Snek"
```

출력을 한 줄씩 읽습니다:

```ts
import { $ } from "snek";

for await (let line of $`printf 'first\nsecond\n'`.lines()) {
  console.log(line); // "first", then "second"
}
```

### 바이너리 데이터 처리

`Buffer`는 바이트 단위로 수정할 수 있습니다. `Blob`은 생성 시점의 데이터를
보관하므로 원본 buffer를 수정해도 바뀌지 않습니다:

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

### S3 접근

자신의 자격 증명과 버킷 이름을 입력하면 객체를 읽고 쓸 수 있습니다:

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

이 다운로드 링크는 한 시간 후 만료됩니다. `method`를 `"PUT"`으로 바꾸면 업로드
링크를 생성합니다.

S3 호환 서비스에 연결하려면 클라이언트 설정에 `endpoint`를 추가합니다.
Cloudflare R2의 예입니다:

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

### 명령줄 인수

`argv`로 명령줄 인수를 가져올 수 있습니다.

```ts
import { argv } from "snek";

console.log(argv); // [ SNEK_PATH, SCRIPT_PATH, "--hello", "world" ]
```

명령줄 인수를 파싱하는 `Cli`를 제공합니다. API는
[cac](https://github.com/cacjs/cac)를 참고했으며 다음과 같이 사용할 수 있습니다:

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

이제 다음과 같이 실행할 수 있습니다.

```sh
snek app.ts --type web --name Alice
snek app.ts --help
snek app.ts --version
snek app.ts -- --name
```
