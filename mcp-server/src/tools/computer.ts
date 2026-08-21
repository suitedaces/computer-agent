// @ts-ignore - no types for screenshot-desktop
import screenshot from "screenshot-desktop";

let nut: any = null;
async function getNut() {
  if (nut !== null) return nut;
  try {
    // @ts-ignore
    const mod = await import("@nut/nut-js");
    nut = mod;
    return nut;
  } catch {
    return null;
  }
}

export async function takeScreenshot(): Promise<string> {
  // screenshot-desktop returns Buffer; encode as base64 jpeg-like (actually png)
  const img = await screenshot({ format: "png" });
  // img is Buffer, convert to base64
  const b64 = (img as Buffer).toString("base64");
  return b64;
}

// AI space 1280x800 -> map to real screen for nut.js
const AI_W = 1280;
const AI_H = 800;

function mapFromAI(x: number, y: number, screenW: number, screenH: number) {
  return {
    x: Math.round((x * screenW) / AI_W),
    y: Math.round((y * screenH) / AI_H),
  };
}

async function getScreenSize(): Promise<{ w: number; h: number }> {
  const n = await getNut();
  if (n) {
    try {
      const w = await n.screen.width();
      const h = await n.screen.height();
      return { w, h };
    } catch {}
  }
  // fallback to 1920x1080 assumption
  return { w: 1920, h: 1080 };
}

export async function mouseMove(x: number, y: number) {
  const n = await getNut();
  if (!n) throw new Error("mouse control requires @nut/nut-js (npm install --include=optional). Fallback: use bash with xdotool / PowerShell.");
  const sz = await getScreenSize();
  const p = mapFromAI(x, y, sz.w, sz.h);
  await n.mouse.setPosition({ x: p.x, y: p.y });
  return `moved to ${p.x},${p.y} (AI ${x},${y})`;
}

export async function click(x: number, y: number, button: "left" | "right" | "middle" = "left", double = false) {
  const n = await getNut();
  if (!n) throw new Error("click requires @nut/nut-js");
  const sz = await getScreenSize();
  const p = mapFromAI(x, y, sz.w, sz.h);
  await n.mouse.setPosition({ x: p.x, y: p.y });
  if (button === "left") {
    await n.mouse.leftClick();
    if (double) await n.mouse.leftClick();
  } else if (button === "right") await n.mouse.rightClick();
  else await n.mouse.leftClick(); // middle not directly, fallback
  return `${double ? "double-" : ""}${button} click at ${p.x},${p.y}`;
}

export async function drag(from: [number, number], to: [number, number]) {
  const n = await getNut();
  if (!n) throw new Error("drag requires @nut/nut-js");
  const sz = await getScreenSize();
  const a = mapFromAI(from[0], from[1], sz.w, sz.h);
  const b = mapFromAI(to[0], to[1], sz.w, sz.h);
  await n.mouse.setPosition({ x: a.x, y: a.y });
  await n.mouse.pressButton(n.Button.LEFT);
  await n.mouse.setPosition({ x: b.x, y: b.y });
  await n.mouse.releaseButton(n.Button.LEFT);
  return `dragged from ${a.x},${a.y} to ${b.x},${b.y}`;
}

export async function typeText(text: string) {
  const n = await getNut();
  if (!n) {
    // fallback via clipboard + paste is not ideal; throw
    throw new Error("type requires @nut/nut-js");
  }
  await n.keyboard.type(text);
  return `typed ${text.length} chars`;
}

export async function pressKey(combo: string) {
  const n = await getNut();
  if (!n) throw new Error("pressKey requires @nut/nut-js");
  // combo like "ctrl+c", "cmd+enter"
  const parts = combo.split("+").map((s) => s.trim().toLowerCase());
  const keyMap: Record<string, any> = {
    enter: n.Key.Enter,
    return: n.Key.Enter,
    tab: n.Key.Tab,
    escape: n.Key.Escape,
    esc: n.Key.Escape,
    space: n.Key.Space,
    backspace: n.Key.Backspace,
    delete: n.Key.Delete,
    up: n.Key.Up,
    down: n.Key.Down,
    left: n.Key.Left,
    right: n.Key.Right,
  };
  const mods: any[] = [];
  let main: any = null;
  for (const p of parts) {
    if (["ctrl", "control"].includes(p)) mods.push(n.Key.LeftControl);
    else if (["alt", "option"].includes(p)) mods.push(n.Key.LeftAlt);
    else if (["shift"].includes(p)) mods.push(n.Key.LeftShift);
    else if (["cmd", "command", "meta", "super"].includes(p)) mods.push(n.Key.LeftSuper);
    else main = keyMap[p] ?? n.Key[p.toUpperCase()] ?? null;
  }
  for (const m of mods) await n.keyboard.pressKey(m);
  if (main) {
    await n.keyboard.pressKey(main);
    await n.keyboard.releaseKey(main);
  } else if (parts.length === 1) {
    // single char
    await n.keyboard.type(parts[0]);
  }
  for (const m of [...mods].reverse()) await n.keyboard.releaseKey(m);
  return `pressed ${combo}`;
}

export async function scroll(x: number, y: number, direction: string, amount = 3) {
  const n = await getNut();
  if (!n) throw new Error("scroll requires @nut/nut-js");
  const sz = await getScreenSize();
  const p = mapFromAI(x, y, sz.w, sz.h);
  await n.mouse.setPosition({ x: p.x, y: p.y });
  const delta = direction === "up" || direction === "left" ? -amount * 100 : amount * 100;
  if (direction === "up" || direction === "down") await n.mouse.scrollUp(delta);
  else await n.mouse.scrollLeft(delta);
  return `scrolled ${direction} ${amount} at ${p.x},${p.y}`;
}
