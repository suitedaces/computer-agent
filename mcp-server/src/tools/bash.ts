import { exec } from "node:child_process";
import { promisify } from "node:util";

const execAsync = promisify(exec);

const BLOCKED = [
  "rm -rf /",
  "rm -rf /*",
  "mkfs",
  "dd if=",
  ":(){:|:&};:",
];

function isBlocked(cmd: string): string | null {
  const low = cmd.toLowerCase();
  for (const p of BLOCKED) if (low.includes(p.toLowerCase())) return p;
  return null;
}

export async function bashRun(command: string, timeoutMs = 30000) {
  const blocked = isBlocked(command);
  if (blocked) throw new Error(`Blocked pattern: ${blocked}`);

  try {
    const { stdout, stderr } = await execAsync(command, {
      timeout: timeoutMs,
      // use bash on unix, powershell on windows is handled via bash -c fallback
      shell: process.platform === "win32" ? "powershell.exe" : "/bin/bash",
      maxBuffer: 5 * 1024 * 1024,
    });
    let out = stdout || "";
    if (stderr) out += (out ? "\n" : "") + `stderr: ${stderr}`;
    if (!out.trim()) out = "(no output)";
    // truncate
    if (out.length > 8000) out = out.slice(0, 8000) + `\n[truncated ${out.length} chars]`;
    return out;
  } catch (e: any) {
    const msg = e.stdout || e.stderr || e.message;
    throw new Error(`bash failed (code ${e.code ?? "?"}): ${msg}`);
  }
}
