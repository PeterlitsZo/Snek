# Snek

Snek ist eine Sammlung praktischer Werkzeuge mit einer
TypeScript-Laufzeitschnittstelle.

Betrachten wir das folgende Skript:

```ts
import { $ } from "snek";

let output = await $`echo "Hello World"`.text();
console.log(JSON.stringify(output));
```

Das Skript führt den Befehl `echo "Hello World"` aus, erfasst die
Standardausgabe und speichert sie in der Variablen `output`. Anschließend kannst
du sie mit `JSON`, `console` und anderen Werkzeugen weiterverarbeiten.

## Installation

Führe den folgenden Befehl aus, um Snek zu installieren:

```bash
curl -fsSL https://peterlits.net/snek/install.sh | bash
```

Standardmäßig wird Snek unter `~/.local/bin/` installiert.

## Referenz

### Befehle ausführen

Führe einen Befehl aus oder lies seine Ausgabe mit `.text()`:

```ts
import { $ } from "snek";

await $`echo "Hello World"`;

let name = "Snek";
let output = await $`echo ${name}`.text();
console.log(output); // "Snek\n"
```

Wenn der Befehl JSON ausgibt, kannst du es direkt parsen:

```ts
import { $ } from "snek";

let data = await $`echo '{"name":"Snek"}'`.json<{ name: string }>();
console.log(data.name); // "Snek"
```

Lies die Ausgabe zeilenweise:

```ts
import { $ } from "snek";

for await (let line of $`printf 'first\nsecond\n'`.lines()) {
  console.log(line); // "first", then "second"
}
```

### Binärdaten verarbeiten

Ein `Buffer` lässt sich byteweise ändern. Ein `Blob` behält die Daten vom
Zeitpunkt seiner Erstellung, auch wenn sich der ursprüngliche Buffer ändert:

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

### Auf S3 zugreifen

Trage deine Zugangsdaten und den Bucket-Namen ein, um Objekte zu lesen und zu
schreiben:

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

Der Download-Link läuft nach einer Stunde ab. Setze `method` auf `"PUT"`, um
einen Upload-Link zu erzeugen.

Für einen S3-kompatiblen Dienst ergänzt du `endpoint` in der
Client-Konfiguration. Zum Beispiel für Cloudflare R2:

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

### Kommandozeilenargumente

Mit `argv` kannst du die Kommandozeilenargumente abrufen.

```ts
import { argv } from "snek";

console.log(argv); // [ SNEK_PATH, SCRIPT_PATH, "--hello", "world" ]
```

Wir stellen `Cli` zum Parsen von Kommandozeilenargumenten bereit. Die API orientiert sich an
[cac](https://github.com/cacjs/cac). So kannst du sie verwenden:

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

Anschließend kannst du das Skript so aufrufen.

```sh
snek app.ts --type web --name Alice
snek app.ts --help
snek app.ts --version
snek app.ts -- --name
```
