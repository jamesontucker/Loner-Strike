// File names the tools refuse before asking the server (it checks again): names Windows keeps
// for devices, the same rule as the app's `windows_unsafe` (setup/record.rs).

export const RESERVED_NAME = "Windows keeps that name for a device (like CON or NUL). Pick another name.";

/** CON, PRN, AUX, NUL, COM1-9, LPT1-9 (any case), whatever follows the first dot. */
export function windowsReserved(name: string): boolean {
  const stem = name.split(".")[0].trimEnd().toUpperCase();
  return ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].includes(stem) || /^(COM|LPT)[1-9]$/.test(stem);
}
