const parseCommand = declaration => {
  if (typeof declaration !== "string") throw new TypeError("Command declaration must be a string");
  const tokens = declaration.trim().split(/\s+/).filter(Boolean);
  const name = tokens.length && !/^[<\[]/.test(tokens[0]) ? tokens.shift() : "";
  if (name && (!/^[^\s<>\[\]]+$/.test(name) || isFlag(name) || name === "!")) {
    throw new Error("Invalid command name: " + name);
  }
  const args = [];
  const names = new Set();
  let optional = false;
  let variadic = false;
  for (const token of tokens) {
    const required = token.startsWith("<") && token.endsWith(">");
    const optionalArgument = token.startsWith("[") && token.endsWith("]");
    const value = token.slice(1, -1);
    const isVariadic = value.startsWith("...");
    const key = isVariadic ? value.slice(3) : value;
    const validName = key.length > 0 && !/[.<>\[\]\s]/.test(key);
    if ((!required && !optionalArgument) || !validName || variadic || (optional && required)) {
      throw new Error("Invalid command declaration: " + declaration);
    }
    variadic = isVariadic;
    if (names.has(key)) throw new Error("Duplicate command argument: " + key);
    names.add(key);
    optional ||= optionalArgument;
    args.push({ required, variadic });
  }
  return { name, args };
};

function isDefaultCommand(command) {
  return command.name === "" || command.aliases.includes("!");
}

function matchesCommand(command, name) {
  if (name === undefined) return false;
  return command.name === name || (name !== "!" && command.aliases.includes(name));
}

function checkCommandName(commands, name) {
  for (const command of commands) {
    const duplicateName = command.name === name || command.aliases.includes(name);
    const duplicateDefault = (name === "!" || name === "") && isDefaultCommand(command);
    if (duplicateName || duplicateDefault) {
      throw new Error("Duplicate command name: " + name);
    }
  }
}

function validateArguments(command, args) {
  if (args.length < command.args.filter(arg => arg.required).length) {
    throw new Error("Missing required arguments for command: " + command.declaration);
  }
  if (!command.args.some(arg => arg.variadic) && args.length > command.args.length) {
    throw new Error("Unused arguments for command: " + command.declaration);
  }
}


function createCommandBuilder(command, commands, registerOption) {
  const builder = {
    option: (flags, description, config = {}) => {
      registerOption(flags, description, config);
      return builder;
    },
    alias: alias => {
      if (typeof alias !== "string" || !alias || /[\s<>\[\]]/.test(alias) || isFlag(alias)) {
        throw new TypeError("Invalid command alias");
      }
      checkCommandName(commands, alias);
      command.aliases.push(alias);
      return builder;
    },
    action: callback => {
      if (typeof callback !== "function") throw new TypeError("Command action must be a function");
      command.action = callback;
      return builder;
    },
  };
  return builder;
}
