#!/usr/bin/env snek

import { $, argv } from "snek";

function check(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

// Get the version.
const args = argv.slice(2);
check(
  args.length === 1,
  "Usage: snek scripts/release.ts VERSION (for example, 0.1.0)",
);
const [version] = args;
check(
  /^[0-9][0-9a-zA-Z.+-]*$/.test(version),
  "Invalid version: use a number first, no v prefix, and only letters, digits, . + -",
);

// Cd to the root of project.
const tag = `v${version}`;
const scriptDir = argv[1].slice(0, argv[1].lastIndexOf("/"));
await $`cd ${scriptDir + "/.."}`;

// Resolve the branch.
const branchResult = await $`git symbolic-ref --quiet --short HEAD`.nothrow();
check(branchResult.exitCode === 0, "Check out a branch before releasing");
const branch = branchResult.stdout.trim();

// Check if the tag is valid.
await $`git check-ref-format refs/tags/${tag}`;

// Check the Cargo.toml file.
interface CargoMetadata {
  packages: { name: string; version: string }[];
}
const metadata: CargoMetadata =
  await $`cargo metadata --no-deps --format-version 1 --locked --offline`.json();
const pkg = metadata.packages.find((pkg) => pkg.name === "snek");
check(pkg, "Could not find the snek package in Cargo metadata");
check(
  version === pkg.version,
  `Update Cargo.toml and Cargo.lock to ${version} before releasing (current version: ${pkg.version})`,
);

// Check the local tags and remote tags.
const localTags = await $`git tag --list ${tag}`.text();
check(!localTags.trim(), `Tag already exists locally: ${tag}`);
const remoteTags =
  await $`git ls-remote --tags origin refs/tags/${tag}`.nothrow();
check(remoteTags.exitCode === 0, "Could not check tags on origin");
check(!remoteTags.stdout.trim(), `Tag already exists on origin: ${tag}`);

// Pulish: add, commit, tag and push.
await $`git add -A`;
await $`git commit --allow-empty -m ${`chore: Release "${tag}".`}`;
await $`git tag ${tag}`;
await $`git push --atomic origin HEAD:refs/heads/${branch} refs/tags/${tag}:refs/tags/${tag}`;

// Happy, happy, happy...
console.log(`Released ${tag} on origin/${branch}`);
