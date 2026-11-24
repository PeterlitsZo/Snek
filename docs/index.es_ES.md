# Snek

Snek es una colección de herramientas prácticas que ofrece una interfaz de
ejecución para TypeScript.

Considera el siguiente script:

```ts
import { $ } from "snek";

let output = await $`echo "Hello World"`.text();
console.log(JSON.stringify(output));
```

El script ejecuta el comando `echo "Hello World"`, captura la salida estándar y
la guarda en la variable `output`. Después, puedes usar `JSON`, `console` y
otras herramientas para procesarla.

## Referencia

### Ejecutar comandos

Ejecuta un comando o usa `.text()` para obtener su salida:

```ts
import { $ } from "snek";

await $`echo "Hello World"`;

let name = "Snek";
let output = await $`echo ${name}`.text();
console.log(output); // "Snek\n"
```

Si el comando devuelve JSON, puedes analizarlo directamente:

```ts
import { $ } from "snek";

let data = await $`echo '{"name":"Snek"}'`.json<{ name: string }>();
console.log(data.name); // "Snek"
```

Lee la salida línea por línea:

```ts
import { $ } from "snek";

for await (let line of $`printf 'first\nsecond\n'`.lines()) {
  console.log(line); // "first", then "second"
}
```

### Trabajar con datos binarios

Puedes modificar un `Buffer` byte a byte. Un `Blob` conserva los datos de su
creación, aunque cambie el buffer original:

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

### Acceder a S3

Introduce tus credenciales y el nombre del bucket para leer y escribir objetos:

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

El enlace de descarga caduca en una hora. Cambia `method` a `"PUT"` para generar
un enlace de subida.

Para conectar con un servicio compatible con S3, añade `endpoint` a la
configuración del cliente. Por ejemplo, para Cloudflare R2:

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

### Argumentos de línea de comandos

Puedes obtener los argumentos de línea de comandos mediante `argv`.

```ts
import { argv } from "snek";

console.log(argv); // [ SNEK_PATH, SCRIPT_PATH, "--hello", "world" ]
```

Ofrecemos `Cli` para analizar los argumentos de línea de comandos. Su API se basa en
[cac](https://github.com/cacjs/cac) y puedes usarla así:

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

Después, puedes interactuar con el script así.

```sh
snek app.ts --type web --name Alice
snek app.ts --help
snek app.ts --version
snek app.ts -- --name
```
