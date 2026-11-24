# Snek

Snek est un ensemble d’outils pratiques offrant une interface d’exécution
TypeScript.

Prenons le script suivant :

```ts
import { $ } from "snek";

let output = await $`echo "Hello World"`.text();
console.log(JSON.stringify(output));
```

Ce script exécute la commande `echo "Hello World"`, capture la sortie standard
et la stocke dans la variable `output`. Vous pouvez ensuite utiliser `JSON`,
`console` ou d’autres outils pour la traiter.

## Référence

### Exécuter des commandes

Exécutez une commande ou récupérez sa sortie avec `.text()` :

```ts
import { $ } from "snek";

await $`echo "Hello World"`;

let name = "Snek";
let output = await $`echo ${name}`.text();
console.log(output); // "Snek\n"
```

Si la commande produit du JSON, vous pouvez le parser directement :

```ts
import { $ } from "snek";

let data = await $`echo '{"name":"Snek"}'`.json<{ name: string }>();
console.log(data.name); // "Snek"
```

Lisez la sortie ligne par ligne :

```ts
import { $ } from "snek";

for await (let line of $`printf 'first\nsecond\n'`.lines()) {
  console.log(line); // "first", then "second"
}
```

### Manipuler des données binaires

Un `Buffer` est modifiable octet par octet. Un `Blob` conserve les données de sa
création, même si le buffer original change :

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

### Accéder à S3

Renseignez vos identifiants et le nom du bucket pour lire et écrire des objets :

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

Ce lien de téléchargement expire après une heure. Passez `method` à `"PUT"` pour
générer un lien d’envoi.

Pour un service compatible S3, ajoutez `endpoint` à la configuration du client.
Par exemple, pour Cloudflare R2 :

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

### Arguments de ligne de commande

Vous pouvez accéder aux arguments de ligne de commande avec `argv`.

```ts
import { argv } from "snek";

console.log(argv); // [ SNEK_PATH, SCRIPT_PATH, "--hello", "world" ]
```

Nous proposons `Cli` pour analyser les arguments de ligne de commande. Son API s’inspire de
[cac](https://github.com/cacjs/cac). Voici comment l’utiliser :

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

Vous pouvez ensuite utiliser le script ainsi.

```sh
snek app.ts --type web --name Alice
snek app.ts --help
snek app.ts --version
snek app.ts -- --name
```
