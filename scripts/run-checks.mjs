// Full local CI for `npm run check`: the same checks as `.github/workflows/ci.yml`
// plus the formatting, naming and duplication checks. Runs every step, then
// lists the failures, so one run shows everything that needs fixing.

import { spawnSync } from "node:child_process";
import process from "node:process";

const denyWarnings = { RUSTFLAGS: "-D warnings" };

/** [label, command, environment overrides]. Commands are fixed strings. */
const steps = [
	["Prettier", "npx prettier --check .", {}],
	["File names (ls-lint)", "npx ls-lint", {}],
	["Duplicate code (jscpd)", "npx jscpd", {}],
	["rustfmt", "cargo fmt --all -- --check", {}],
	[
		"Clippy",
		"cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::all -W clippy::style",
		{},
	],
	[
		"Build, warnings denied",
		"cargo check --workspace --all-targets --all-features --locked",
		denyWarnings,
	],
	[
		"uad-core alone",
		"cargo check --all-targets --locked -p uad-core",
		denyWarnings,
	],
	[
		"uad-cli alone",
		"cargo check --all-targets --locked -p uad-cli",
		denyWarnings,
	],
	[
		"uad-gui without self-update",
		"cargo check --all-targets --locked -p uad-gui --no-default-features --features wgpu,no-self-update,img",
		denyWarnings,
	],
	[
		"Screenshot stand-in adb",
		"cargo fmt --manifest-path scripts/screenshots/demo-adb/Cargo.toml -- --check && cargo clippy --manifest-path scripts/screenshots/demo-adb/Cargo.toml --target-dir target/demo-adb --locked -- -D warnings -D clippy::all -W clippy::style",
		{},
	],
	["Tests", "cargo test --workspace --all-features --locked", {}],
];

const failures = [];
for (const [label, command, env] of steps) {
	console.log(`\n[check] ${label}: ${command}`);
	const result = spawnSync(command, {
		shell: true,
		stdio: "inherit",
		env: { ...process.env, ...env },
	});
	if (result.status !== 0) {
		failures.push(label);
	}
}

if (failures.length > 0) {
	console.error(`\n[check] Failed: ${failures.join(", ")}`);
	process.exit(1);
}
console.log(`\n[check] All ${steps.length} checks passed.`);
