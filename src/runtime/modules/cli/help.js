function showHelp(script, version, commands, command, definitions) {
  let usage = " [options] [args...]";
  if (command) usage = " " + command.declaration + " [options]";
  else if (commands.length) usage = " <command> [options]";
  const lines = [
    script + (version === undefined ? "" : " v" + version),
    "",
    "Usage: snek " + script + usage,
    "",
  ];
  if ((!command || isDefaultCommand(command)) && commands.length) {
    lines.push("Commands:");
    for (const listed of commands) {
      lines.push("  " + listed.declaration + "  " + listed.description);
    }
    lines.push("");
  }
  lines.push("Options:");
  const width = definitions.reduce(
    (width, definition) => Math.max(width, definition.flags.length), 0,
  );
  for (const definition of definitions) {
    const suffix = definition.hasDefault
      ? " (default: " + JSON.stringify(definition.defaultValue) + ")" : "";
    lines.push("  " + definition.flags.padEnd(width) + "  " + definition.description + suffix);
  }
  print(lines.join("\n"));
}
