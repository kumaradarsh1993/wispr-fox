export type AvatarState =
  | "idle"
  | "listening"
  | "thinking"
  | "writing"
  | "pasting"
  | "error"
  | "sleeping"
  | "excited";

export type RasterAvatarSkin = "codex-fox" | "oru-gujia" | "spark-buddy";

export type AvatarArt = {
  w: number;
  h: number;
  head: number;
};

export type RasterAvatarPack = {
  manifestVersion: 2;
  renderer: "raster-state-pack";
  id: RasterAvatarSkin;
  name: string;
  description: string;
  basePath: string;
  thumbnail: string;
  art: AvatarArt;
  states: Record<AvatarState, string>;
  bubbleSkin: RasterAvatarSkin;
};

export const RASTER_AVATAR_STATES: readonly AvatarState[] = [
  "idle",
  "listening",
  "thinking",
  "writing",
  "pasting",
  "error",
  "sleeping",
  "excited",
] as const;

function makeStates(id: RasterAvatarSkin): Record<AvatarState, string> {
  return {
    idle: `/avatars/${id}/idle.png`,
    listening: `/avatars/${id}/listening.png`,
    thinking: `/avatars/${id}/thinking.png`,
    writing: `/avatars/${id}/writing.png`,
    pasting: `/avatars/${id}/pasting.png`,
    error: `/avatars/${id}/error.png`,
    sleeping: `/avatars/${id}/sleeping.png`,
    excited: `/avatars/${id}/excited.png`,
  };
}

export const RASTER_AVATAR_PACKS: Record<RasterAvatarSkin, RasterAvatarPack> = {
  "codex-fox": {
    manifestVersion: 2,
    renderer: "raster-state-pack",
    id: "codex-fox",
    name: "Codex Fox",
    description: "Polished 2.5D fox companion with Codex-blue collar glow.",
    basePath: "/avatars/codex-fox",
    thumbnail: "/avatars/codex-fox/thumbnail.png",
    art: { w: 170, h: 182, head: 182 },
    states: makeStates("codex-fox"),
    bubbleSkin: "codex-fox",
  },
  "oru-gujia": {
    manifestVersion: 2,
    renderer: "raster-state-pack",
    id: "oru-gujia",
    name: "Oru & Gujia",
    description: "Personal two-cat avatar pack based on Oru and Gujia.",
    basePath: "/avatars/oru-gujia",
    thumbnail: "/avatars/oru-gujia/thumbnail.png",
    art: { w: 222, h: 214, head: 214 },
    states: makeStates("oru-gujia"),
    bubbleSkin: "oru-gujia",
  },
  "spark-buddy": {
    manifestVersion: 2,
    renderer: "raster-state-pack",
    id: "spark-buddy",
    name: "Spark Buddy",
    description: "Original electric companion with lively 2.5D poses.",
    basePath: "/avatars/spark-buddy",
    thumbnail: "/avatars/spark-buddy/thumbnail.png",
    art: { w: 168, h: 184, head: 184 },
    states: makeStates("spark-buddy"),
    bubbleSkin: "spark-buddy",
  },
};

export const RASTER_AVATAR_ART: Record<RasterAvatarSkin, AvatarArt> = {
  "codex-fox": RASTER_AVATAR_PACKS["codex-fox"].art,
  "oru-gujia": RASTER_AVATAR_PACKS["oru-gujia"].art,
  "spark-buddy": RASTER_AVATAR_PACKS["spark-buddy"].art,
};

export function isRasterAvatarSkin(skin: string): skin is RasterAvatarSkin {
  return skin in RASTER_AVATAR_PACKS;
}

export function rasterAvatarForSkin(skin: string): RasterAvatarPack | undefined {
  return (RASTER_AVATAR_PACKS as Record<string, RasterAvatarPack | undefined>)[skin];
}
