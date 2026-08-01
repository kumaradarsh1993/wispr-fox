<script lang="ts">
  // Sprite-sheet pet renderer. One div, the sheet as background-image,
  // background-position stepped through the active animation row on an
  // fps timer. Display size = frame size × PET_ZOOM × --fscale (S/M/L
  // slider), scaled via background-size so layout and paint agree — no
  // transforms, so the floater's box math stays honest.
  import { PET_SHEET, PET_ANIMS, petSheetRows, type PetAnimName } from "./pets";

  let { petId, anim = "idle" } = $props<{ petId: string; anim?: PetAnimName }>();

  // Base zoom: 192×208 source frames render at ~111×121 logical px, in the
  // same size family as the other character avatars. Must track the pet ART
  // entry in clippy/+page.svelte.
  const PET_ZOOM = 0.58;

  let frame = $state(0);

  // Step frames on the active animation's fps clock; restart from frame 0
  // whenever the animation changes so enters (e.g. celebration) play from
  // the top. All rows loop — state dwell times decide how long they run.
  $effect(() => {
    const spec = PET_ANIMS[anim as PetAnimName] ?? PET_ANIMS.idle;
    frame = 0;
    const t = setInterval(() => {
      frame = (frame + 1) % spec.frames;
    }, Math.round(1000 / spec.fps));
    return () => clearInterval(t);
  });

  let row = $derived((PET_ANIMS[anim as PetAnimName] ?? PET_ANIMS.idle).row);
  let sheetHeight = $derived(PET_SHEET.fh * petSheetRows(petId));
</script>

<div
  class="sprite-pet"
  style="
    --pz: calc({PET_ZOOM} * var(--fscale, 1));
    background-image: url('/pets/{petId}.webp');
    background-position: calc({-frame * PET_SHEET.fw}px * var(--pz)) calc({-row * PET_SHEET.fh}px * var(--pz));
    background-size: calc(1536px * var(--pz)) calc({sheetHeight}px * var(--pz));
  "
  aria-hidden="true"
></div>

<style>
  .sprite-pet {
    width: calc(192px * var(--pz));
    height: calc(208px * var(--pz));
    background-repeat: no-repeat;
    /* Whole-sheet size is set inline because v2 hatch-pets have 11 rows. */
    flex: 0 0 auto;
  }
</style>
