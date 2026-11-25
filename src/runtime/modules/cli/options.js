const isValue = value => typeof value === "string" || typeof value === "boolean";
const isFlag = value => value.startsWith("-") && value !== "-" &&
  !/^-\d+(?:\.\d+)?$/.test(value);
const keyOf = name => name.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());

const parseOptionDeclaration = flags => {
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

function createOption(flags, description, config, kind) {
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

  const { names, key, mode } = parseOptionDeclaration(flags);

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

  return definition;
}

function checkOptionConflicts(option, definitions) {
  for (const existing of definitions) {
    const sharesName = option.names.some(name => existing.names.includes(name));
    if (existing.key === option.key || sharesName) {
      throw new Error("Duplicate option declaration: " + option.flags);
    }
  }
}

function validateOptions(options, definitions) {
  if (!options || typeof options !== "object" || Array.isArray(options)) {
    throw new TypeError("Command options must be an object");
  }
  for (const key of Object.keys(options)) {
    const definition = definitions.find(definition => definition.key === key);
    if (!definition) throw new Error("Unknown option: " + key);
    const value = options[key];
    if (!isValue(value)) throw new TypeError("Invalid option value: " + key);
    if (definition.mode === "boolean" && typeof value !== "boolean") {
      throw new TypeError("Boolean option requires a boolean: " + key);
    }
  }
}
