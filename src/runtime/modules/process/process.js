(argv, host) => {
  const env = new Proxy(Object.create(null), {
    get: (_, key) => typeof key === "string" ? host("get", key) : undefined,
    has: (_, key) => typeof key === "string" && host("get", key) !== undefined,
    ownKeys: () => host("keys"),
    getOwnPropertyDescriptor: (_, key) => {
      const value = typeof key === "string" ? host("get", key) : undefined;
      return value === undefined ? undefined : { value, writable: true, enumerable: true, configurable: true };
    },
    set: (_, key, value) => { host("set", key, value); return true; },
    deleteProperty: (_, key) => { host("delete", key); return true; },
    defineProperty: (_, key, descriptor) => {
      if (!("value" in descriptor) || !descriptor.writable || !descriptor.enumerable || !descriptor.configurable)
        throw new TypeError("process.env requires configurable, enumerable, writable data properties");
      host("set", key, descriptor.value);
      return true;
    },
    preventExtensions: () => false,
    setPrototypeOf: (_, prototype) => prototype === null,
  });
  const process = {};
  for (const [key, value] of Object.entries({ argv, env, ...host("metadata") })) {
    Object.defineProperty(process, key, { value, enumerable: true });
  }
  Object.defineProperties(process, {
    cwd: { value: () => host("cwd"), enumerable: true },
    chdir: { value: path => { host("chdir", path); }, enumerable: true },
    exit: { value: code => { host("exit", code); }, enumerable: true },
    exitCode: { get: () => host("exitCode"), set: value => { host("exitCode", value); }, enumerable: true },
  });
  return process;
}
