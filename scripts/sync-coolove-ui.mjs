import { copyFile, mkdir, readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
const source = new URL("../../coolove-ui/", import.meta.url);
const destination = new URL("../src/coolove-ui/", import.meta.url);
await mkdir(destination, { recursive: true });
for (const [from, to] of [
  ["coolove-ui.css", "coolove-ui.css"],
  ["vue/useCooloveTheme.ts", "useCooloveTheme.ts"],
  ["tauri/nativeGlass.ts", "nativeGlass.ts"],
]) {
  await copyFile(new URL(from, source), new URL(to, destination));
}
const css = await readFile(new URL("coolove-ui.css", destination), "utf8");
console.log(
  `Synced ${css.match(/Coolove UI v[\d.]+/)?.[0]} to ${fileURLToPath(destination)}`,
);
