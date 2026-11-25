function parseTokens(input, definitions) {
  const names = new Map();
  for (const definition of definitions) {
    for (const name of definition.names) names.set(name, definition);
  }
  let commandName;
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
  for (const definition of definitions) {
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
      if (!args.length) commandName = token;
      args.push(token);
      continue;
    }
    const equal = token.indexOf("=");
    const name = equal < 0 ? token : token.slice(0, equal);
    const definition = names.get(name);
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
  return { args, options, error, help, version, commandName };
}
