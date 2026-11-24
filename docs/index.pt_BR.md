# Snek

Snek é uma coleção de ferramentas práticas que oferece uma interface de execução
para TypeScript.

Considere o seguinte script:

```ts
import { $ } from "snek";

let output = await $`echo "Hello World"`.text();
console.log(JSON.stringify(output));
```

O script executa o comando `echo "Hello World"`, captura a saída padrão e a
armazena na variável `output`. Depois, você pode usar `JSON`, `console` e outras
ferramentas para processá-la.

## Referência

### Executar comandos

Execute um comando ou use `.text()` para obter a saída:

```ts
import { $ } from "snek";

await $`echo "Hello World"`;

let name = "Snek";
let output = await $`echo ${name}`.text();
console.log(output); // "Snek\n"
```

Se o comando produzir JSON, você pode interpretá-lo diretamente:

```ts
import { $ } from "snek";

let data = await $`echo '{"name":"Snek"}'`.json<{ name: string }>();
console.log(data.name); // "Snek"
```

Leia a saída linha por linha:

```ts
import { $ } from "snek";

for await (let line of $`printf 'first\nsecond\n'`.lines()) {
  console.log(line); // "first", then "second"
}
```

### Trabalhar com dados binários

Um `Buffer` pode ser alterado byte a byte. Um `Blob` mantém os dados da criação,
mesmo que o buffer original seja alterado:

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

### Acessar o S3

Preencha suas credenciais e o nome do bucket para ler e gravar objetos:

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

O link de download expira em uma hora. Altere `method` para `"PUT"` para gerar
um link de upload.

Para conectar a um serviço compatível com S3, adicione `endpoint` à configuração
do cliente. Por exemplo, para o Cloudflare R2:

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

### Argumentos de linha de comando

Você pode obter os argumentos de linha de comando com `argv`.

```ts
import { argv } from "snek";

console.log(argv); // [ SNEK_PATH, SCRIPT_PATH, "--hello", "world" ]
```

Oferecemos `Cli` para analisar os argumentos de linha de comando. Sua API se inspira em
[cac](https://github.com/cacjs/cac) e pode ser usada assim:

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

Depois, você pode interagir com o script assim.

```sh
snek app.ts --type web --name Alice
snek app.ts --help
snek app.ts --version
snek app.ts -- --name
```
