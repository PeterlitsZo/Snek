#!/usr/bin/env snek

import { $, argv } from "snek";

function check(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

const args = argv.slice(2);
check(args.length === 1, "Usage: snek scripts/release.ts VERSION (for example, 0.1.0)");

const [version] = args;
check(/^[0-9][0-9a-zA-Z.+-]*$/.test(version), "Invalid version: use a number first, no v prefix, and only letters, digits, . + -");

const tag = `v${version}`;
const root = argv[1].slice(0, argv[1].lastIndexOf("/")) + "/..";
const branch = await $`git -C ${root} symbolic-ref --quiet --short HEAD`
  .text()
  .then(value => value.trim(), () => {
    throw new Error("Check out a branch before releasing");
  });
await $`git -C ${root} check-ref-format refs/tags/${tag}`;

interface CargoMetadata {
  packages: { name: string; version: string }[];
}

const metadata: CargoMetadata = await $`cargo metadata --manifest-path ${root + "/Cargo.toml"} --no-deps --format-version 1 --locked --offline`.json();
const pkg = metadata.packages.find(pkg => pkg.name === "snek");
check(pkg, "Could not find the snek package in Cargo metadata");
check(version === pkg.version, `Update Cargo.toml and Cargo.lock to ${version} before releasing (current version: ${pkg.version})`);

const localTags = await $`git -C ${root} tag --list ${tag}`.text();
check(!localTags.trim(), `Tag already exists locally: ${tag}`);
const remoteTags = await $`git -C ${root} ls-remote --tags origin refs/tags/${tag}`
  .text()
  .catch(() => {
    throw new Error("Could not check tags on origin");
  });
check(!remoteTags.trim(), `Tag already exists on origin: ${tag}`);

await $`git -C ${root} add -A`;
await $`git -C ${root} commit --allow-empty -m ${`chore: Release "${tag}".`}`;
await $`git -C ${root} tag ${tag}`;
await $`git -C ${root} push --atomic origin HEAD:refs/heads/${branch} refs/tags/${tag}:refs/tags/${tag}`;

console.log(`Released ${tag} on origin/${branch}`);
