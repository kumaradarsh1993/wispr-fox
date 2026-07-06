// Terminal pet sprite-sheet catalog — the Codex CLI pets, rendered by
// SpritePet.svelte as floater avatars. Sheet format reverse-engineered from
// the Codex CLI: every sheet is 1536×1872 WEBP,
// 8 columns × 9 rows of 192×208 frames, rows = animations. All eight
// built-in pets share the exact same grid and frame counts (verified by
// alpha-scanning every sheet), so ONE animation table serves them all.
//
// Assets live in static/pets/ — see the attribution README there.

export const PET_SHEET = { fw: 192, fh: 208, cols: 8, rows: 9 } as const;

export type PetAnimName =
  | "idle"
  | "run-right"
  | "run-left"
  | "waving"
  | "jumping"
  | "sad"
  | "waiting"
  | "typing"
  | "celebration";

export const PET_ANIMS: Record<PetAnimName, { row: number; frames: number; fps: number }> = {
  idle:          { row: 0, frames: 6, fps: 5 },
  "run-right":   { row: 1, frames: 8, fps: 10 },
  "run-left":    { row: 2, frames: 8, fps: 10 },
  waving:        { row: 3, frames: 4, fps: 6 },
  jumping:       { row: 4, frames: 5, fps: 8 },
  sad:           { row: 5, frames: 8, fps: 6 },
  waiting:       { row: 6, frames: 6, fps: 6 },
  typing:        { row: 7, frames: 6, fps: 9 },
  celebration:   { row: 8, frames: 6, fps: 8 },
};

export const PETS = [
  { id: "codex",       label: "Codex Pet",   desc: "The original Codex companion — a cloud-headed terminal robot" },
  { id: "dewey",       label: "Dewey",       desc: "A tidy duck for calm workspace days" },
  { id: "fireball",    label: "Fireball",    desc: "Hot path energy for fast iteration" },
  { id: "rocky",       label: "Rocky",       desc: "A steady rock when the diff gets large" },
  { id: "seedy",       label: "Seedy",       desc: "Small green shoots for new ideas" },
  { id: "stacky",      label: "Stacky",      desc: "A balanced stack for deep work" },
  { id: "bsod",        label: "BSOD",        desc: "A tiny blue-screen gremlin" },
  { id: "null-signal", label: "Null Signal", desc: "Quiet signal from the void" },
] as const;

export type PetId = (typeof PETS)[number]["id"];

/** Skin ids are namespaced "pet-<id>" so the pet family reads as one group. */
export const PET_SKINS = PETS.map((p) => `pet-${p.id}`);

export function isPetSkin(skin: string): boolean {
  return skin.startsWith("pet-");
}

export function petIdFromSkin(skin: string): string {
  return skin.slice(4);
}
