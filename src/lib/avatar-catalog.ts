import type { Skin } from "./skin-store.svelte";

export type AvatarOption = {
  id: Skin;
  label: string;
  description: string;
  mark: string;
  family: "wispr" | "codex" | "classic" | "minimal" | "pet";
};

/** User-facing avatar identity. Persisted skin ids remain unchanged. */
export const AVATAR_OPTIONS: readonly AvatarOption[] = [
  { id: "fox", label: "Fox", description: "The watercolor wispr-fox mascot, with a different pose for every stage of dictation.", mark: "Fx", family: "wispr" },
  { id: "codex-fox", label: "Codex Fox", description: "High-fidelity 2.5D fox with a Codex-blue glow and state-specific poses.", mark: "CF", family: "codex" },
  { id: "stylized", label: "Clippo", description: "The black paperclip: bold, compact, expressive, and unmistakably different from classic Clippy.", mark: "Co", family: "wispr" },
  { id: "real-clippy", label: "Clippy", description: "Classic Microsoft Clippy with the original animated character artwork.", mark: "Cl", family: "classic" },
  { id: "cat", label: "Blacky", description: "A charcoal-black desk cat with green eyes, typing paws, and a quietly watchful personality.", mark: "Bk", family: "wispr" },
  { id: "oru-gujia", label: "Uru & Gujia", description: "The Codex-authored two-cat team: Gujia supervises while Uru handles the typing.", mark: "UG", family: "codex" },
  { id: "pet-mochi-marmalade", label: "Mochi & Marmalade", description: "A newer Codex v2 duo: a graceful white cat and curious ginger tabby, always moving together.", mark: "MM", family: "codex" },
  { id: "spark-buddy", label: "Pikachu", description: "The Codex-authored electric yellow avatar with lively poses, teal cheek glow, and celebratory sparks.", mark: "Pk", family: "codex" },
  { id: "wave", label: "Wavy", description: "A minimal live waveform in a polished floating pill, without a character or speech bubbles.", mark: "≈", family: "minimal" },
  { id: "siri", label: "Siri", description: "A luminous multicolour voice orb that quickens, blooms, and settles with the dictation state.", mark: "◉", family: "minimal" },
  { id: "pet-codex", label: "Codex Pet", description: "The original Codex terminal companion: a cloud-headed robot.", mark: "Px", family: "pet" },
  { id: "pet-dewey", label: "Dewey", description: "A tidy duck for calm workspace days.", mark: "Dw", family: "pet" },
  { id: "pet-fireball", label: "Fireball", description: "Hot-path energy for fast dictation.", mark: "Fb", family: "pet" },
  { id: "pet-rocky", label: "Rocky", description: "A steady rock when the monologue gets long.", mark: "Rk", family: "pet" },
  { id: "pet-seedy", label: "Seedy", description: "Small green shoots for new ideas.", mark: "Sd", family: "pet" },
  { id: "pet-stacky", label: "Stacky", description: "A balanced stack for deep work.", mark: "St", family: "pet" },
  { id: "pet-bsod", label: "BSOD", description: "A tiny blue-screen gremlin.", mark: "Bs", family: "pet" },
  { id: "pet-null-signal", label: "Null Signal", description: "A quiet signal from the void.", mark: "Ns", family: "pet" },
] as const;

export function avatarOption(skin: Skin): AvatarOption | undefined {
  return AVATAR_OPTIONS.find((option) => option.id === skin);
}

export function avatarLabel(skin: Skin): string {
  return avatarOption(skin)?.label ?? skin;
}
