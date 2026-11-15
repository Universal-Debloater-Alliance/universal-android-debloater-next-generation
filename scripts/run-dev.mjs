// Local launcher for `npm run dev`: builds and starts the UAD-ng GUI in debug mode.
//
// Cargo builds on the first run and rebuilds only what changed on later runs,
// so there is no separate setup step. Ctrl+C stops the app and the build.
// Extra arguments go to the app: `npm run dev -- --some-flag`.

import { spawn, spawnSync } from "node:child_process";
import process from "node:process";

/** Returns true when `command --version` runs. */
function isInstalled(command) {
	const result = spawnSync(command, ["--version"], { stdio: "ignore" });
	return result.error === undefined && result.status === 0;
}

if (!isInstalled("cargo")) {
	console.error(
		"[dev] Rust is not installed or `cargo` is not on PATH. Install it from https://rustup.rs and run `npm run dev` again.",
	);
	process.exit(1);
}

if (!isInstalled("adb")) {
	console.warn(
		"[dev] `adb` was not found on PATH. The app starts, but it cannot list or change devices until Android platform-tools are installed.",
	);
}

console.log(
	"[dev] Building and starting the GUI (`cargo run -p uad-gui`). The first build takes a few minutes.",
);
console.log(
	"[dev] Logs: the terminal and the `uadng.log` file in the UAD cache folder.",
);

const app = spawn(
	"cargo",
	["run", "-p", "uad-gui", "--", ...process.argv.slice(2)],
	{
		stdio: "inherit",
	},
);

/** Stops the app and its build when the launcher is interrupted. */
function stopApp(signal) {
	if (app.exitCode === null && app.signalCode === null) {
		app.kill(signal);
	}
}
for (const signal of ["SIGINT", "SIGTERM", "SIGBREAK"]) {
	process.on(signal, () => stopApp(signal));
}

app.on("error", (error) => {
	console.error(`[dev] Could not start cargo: ${error.message}`);
	process.exit(1);
});

app.on("exit", (code, signal) => {
	if (signal !== null) {
		console.log(`[dev] Stopped (${signal}).`);
		process.exit(0);
	}
	if (code !== 0) {
		console.error(
			`[dev] cargo exited with code ${code}. The output above shows the cause.`,
		);
	}
	process.exit(code ?? 1);
});
