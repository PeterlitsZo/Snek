const print = console.log.bind(console);

class Cli {
  #definitions = [];
  #version;
  #commands = [];
  #matched;
  args = [];
  options = Object.create(null);

  command(declaration, description = "") {
    if (typeof description !== "string") throw new TypeError("Command description must be a string");
    const { name, args } = parseCommand(declaration);
    const command = {
      name,
      args,
      declaration: declaration.trim(),
      description,
      aliases: [],
      definitions: [],
    };
    checkCommandName(this.#commands, name);
    this.#commands.push(command);
    return createCommandBuilder(command, this.#commands, (flags, description, config) => {
      this.#register(flags, description, config, "option", command);
    });
  }

  runMatchedCommand() {
    const command = this.#matched;
    if (!command) return;
    if (!Array.isArray(this.args)) throw new TypeError("Command arguments must be strings");
    for (const value of this.args) {
      if (typeof value !== "string") throw new TypeError("Command arguments must be strings");
    }
    validateArguments(command, this.args);
    validateOptions(this.options, this.#definitionsFor(command));
    if (!command.action) return;
    const values = command.args.map((arg, index) => {
      return arg.variadic ? this.args.slice(index) : this.args[index];
    });
    return command.action.apply(this, [...values, this.options]);
  }

  #definitionsFor(command) {
    if (!command) return this.#definitions;
    return [...this.#definitions, ...command.definitions];
  }

  #register(flags, description, config, kind, command) {
    const definition = createOption(flags, description, config, kind);
    checkOptionConflicts(definition, this.#definitionsFor(command));
    if (!command) {
      for (const existing of this.#commands) {
        checkOptionConflicts(definition, existing.definitions);
      }
    }
    const target = command ? command.definitions : this.#definitions;
    target.push(definition);
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

  #matchCommand(input) {
    const matches = [];
    for (const command of this.#commands) {
      const parsed = parseTokens(input, this.#definitionsFor(command));
      if (matchesCommand(command, parsed.commandName)) {
        parsed.args.shift();
        matches.push({ command, parsed });
      }
    }
    if (matches.length > 1) throw new Error("Ambiguous command");
    if (matches.length === 1) return matches[0];
    const command = this.#commands.find(isDefaultCommand);
    const parsed = parseTokens(input, this.#definitionsFor(command));
    return { command, parsed };
  }

  parse(input = argv.slice(2), config = {}) {
    if (!Array.isArray(input)) throw new TypeError("parse expects an array of strings");
    for (const value of input) {
      if (typeof value !== "string") throw new TypeError("parse expects an array of strings");
    }
    if (!config || typeof config !== "object" || Array.isArray(config) ||
      (config.run !== undefined && typeof config.run !== "boolean")) {
      throw new TypeError("Invalid parse configuration");
    }
    const { command, parsed } = this.#matchCommand(input);
    const { args, options, help, version, error } = parsed;
    if (!help && !version) {
      if (error) throw error;
      if (command) validateArguments(command, args);
      else if (this.#commands.length && args.length) throw new Error("Unknown command: " + args[0]);
    }
    this.args = args;
    this.options = options;
    this.#matched = help || version ? undefined : command;
    if (help) {
      showHelp(script, this.#version, this.#commands, command, this.#definitionsFor(command));
    } else if (version) {
      print(this.#version);
    } else if (config.run !== false && this.#matched) {
      trackAction(this.runMatchedCommand());
    }
    return { args, options };
  }
}

return Cli;
