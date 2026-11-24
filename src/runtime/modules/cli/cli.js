(argv, script) => {
  const print = console.log.bind(console);
  const isValue = value => typeof value === "string" || typeof value === "boolean";
  const isFlag = value => value.startsWith("-") && value !== "-" &&
    !/^-\d+(?:\.\d+)?$/.test(value);
  const keyOf = name => name.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());

  const parseDeclaration = flags => {
    const invalid = () => new Error("Invalid option declaration: " + flags);
    let aliases = flags.trim();
    let mode = "boolean";
    if (aliases.endsWith(">") || aliases.endsWith("]")) {
      const closing = aliases.at(-1);
      const opening = closing === ">" ? "<" : "[";
      const start = aliases.lastIndexOf(opening);
      if (start <= 0 || !/\s/.test(aliases[start - 1])) throw invalid();
      const placeholder = aliases.slice(start + 1, -1);
      if (!placeholder || /[<>\[\]\s]/.test(placeholder)) throw invalid();
      mode = opening === "<" ? "required" : "optional";
      aliases = aliases.slice(0, start).trimEnd();
    }

    const names = aliases.split(",").map(name => name.trim());
    let longName;
    let shortName;
    for (const name of names) {
      if (name.startsWith("--")) {
        if (longName || !/^--[a-zA-Z][a-zA-Z0-9-]*$/.test(name)) throw invalid();
        longName = name;
      } else {
        if (shortName || !/^-[a-zA-Z]$/.test(name)) throw invalid();
        shortName = name;
      }
    }
    const name = longName ? longName.slice(2) : shortName.slice(1);
    return { names, key: keyOf(name), mode };
  };

  return class Cli {
    #definitions = [];
    #names = new Map();
    #version;
    args = [];
    options = Object.create(null);

    #register(flags, description, config, kind) {
      if (typeof flags !== "string") {
        throw new TypeError("Option flags must be a string");
      }
      if (typeof description !== "string") {
        throw new TypeError("Option description must be a string");
      }
      if (config === null || typeof config !== "object") {
        throw new TypeError("Option config must be an object");
      }
      if (Array.isArray(config)) {
        throw new TypeError("Option config must not be an array");
      }

      const { names, key, mode } = parseDeclaration(flags);

      for (const name of names) {
        if (this.#names.has(name)) {
          throw new Error("Duplicate option declaration: " + flags);
        }
      }
      if (this.#definitions.some(definition => definition.key === key)) {
        throw new Error("Duplicate option declaration: " + flags);
      }

      const hasDefault = Object.hasOwn(config, "default");
      const defaultValue = hasDefault ? config.default : undefined;
      if (hasDefault && !isValue(defaultValue)) {
        throw new TypeError("Option default must be a string or boolean");
      }
      if (mode === "boolean" && hasDefault && typeof defaultValue !== "boolean") {
        throw new TypeError("Boolean option default must be a boolean");
      }

      const definition = {
        flags: flags.trim(),
        description,
        names,
        key,
        mode,
        kind,
        hasDefault,
        defaultValue,
      };

      this.#definitions.push(definition);

      for (const name of names) this.#names.set(name, definition);

      return this;
    }

    option(flags, description, config = {}) {
      return this.#register(flags, description, config, "option");
    }

    help() {
      return this.#register("-h, --help", "Display this help message", {}, "help");
    }

    version(value) {
      if (typeof value !== "string") throw new TypeError("Version must be a string");
      this.#register("-v, --version", "Display the version number", {}, "version");
      this.#version = value;
      return this;
    }

    #showHelp() {
      const lines = [
        script + (this.#version === undefined ? "" : " v" + this.#version),
        "",
        "Usage: snek " + script + " [options] [args...]",
        "",
        "Options:",
      ];
      const width = this.#definitions.reduce(
        (width, definition) => Math.max(width, definition.flags.length), 0,
      );
      for (const definition of this.#definitions) {
        const suffix = definition.hasDefault
          ? " (default: " + JSON.stringify(definition.defaultValue) + ")" : "";
        lines.push("  " + definition.flags.padEnd(width) + "  " + definition.description + suffix);
      }
      print(lines.join("\n"));
    }

    parse(input = argv.slice(2)) {
      if (!Array.isArray(input)) {
        throw new TypeError("parse expects an array of strings");
      }
      // Iteration visits sparse slots as undefined; Array.some would skip them.
      for (const value of input) {
        if (typeof value !== "string") {
          throw new TypeError("parse expects an array of strings");
        }
      }
      // Build fresh results locally. A failed parse preserves the previous
      // successful result, and option names cannot inherit Object properties.
      const args = [];
      const options = Object.create(null);
      let error;
      let help = false;
      let version = false;
      // Defer ordinary errors so a later help/version flag can still work.
      // Recognize those flags during parsing, not by scanning raw tokens:
      // "--name=--help" and tokens after "--" must not trigger help.
      const fail = message => { error ??= new Error(message); };
      for (const definition of this.#definitions) {
        if (definition.hasDefault) options[definition.key] = definition.defaultValue;
      }
      for (let index = 0; index < input.length; index++) {
        const token = input[index];
        if (token === "--") {
          // Stop option processing without a spread-call argument limit.
          for (index++; index < input.length; index++) args.push(input[index]);
          break;
        }
        if (!isFlag(token)) {
          args.push(token);
          continue;
        }
        const equal = token.indexOf("=");
        const name = equal < 0 ? token : token.slice(0, equal);
        const definition = this.#names.get(name);
        if (!definition) {
          fail("Unknown option: " + name);
          continue;
        }
        if (definition.mode === "boolean") {
          if (equal >= 0) {
            fail("Option does not accept a value: " + name);
            continue;
          }
          options[definition.key] = true;
          help ||= definition.kind === "help";
          version ||= definition.kind === "version";
          continue;
        }
        // Explicit assignment preserves empty strings and leading dashes.
        // Later occurrences overwrite earlier values, including defaults.
        if (equal >= 0) {
          options[definition.key] = token.slice(equal + 1);
        } else if (index + 1 < input.length && !isFlag(input[index + 1])) {
          options[definition.key] = input[++index];
        } else if (definition.mode === "optional") {
          options[definition.key] = true;
        } else {
          fail("Missing value for option: " + name);
        }
      }
      if (!help && !version && error) throw error;
      this.args = args;
      this.options = options;
      // Help wins over version. Returning normally lets callers decide
      // whether their application logic should continue after either output.
      if (help) this.#showHelp();
      else if (version) print(this.#version);
      return { args, options };
    }
  };
}
