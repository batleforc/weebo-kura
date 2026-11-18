<script setup lang="ts">
/** The shared folders: closed unless opened, or on the way to the open note. */
import Chevron from "@/components/ui/Chevron.vue";
import type { Folder } from "./tree";
import { noteName } from "./tree";

defineProps<{
  folder: Folder;
  vault: string;
  current?: string;
  open: ReadonlySet<string>;
}>();
const emit = defineEmits<{ toggle: [path: string, open: boolean] }>();

function toggled(path: string, event: Event): void {
  if (event.currentTarget instanceof HTMLDetailsElement)
    emit("toggle", path, event.currentTarget.open);
}
</script>

<template>
  <ul>
    <li v-for="f in folder.folders" :key="f.path">
      <details :open="open.has(f.path)" :data-folder="f.path" @toggle="toggled(f.path, $event)">
        <summary class="flex cursor-pointer list-none gap-1 px-2 py-1 text-ink-dim hover:text-ink [&::-webkit-details-marker]:hidden">
          <Chevron :dir="open.has(f.path) ? 'down' : 'right'" class="mt-[0.3rem] transition-transform duration-150" />
          <span class="min-w-0 break-words">{{ f.name }}</span>
        </summary>
        <FolderTree class="ml-[0.6875rem] border-l border-rule-soft pl-1" :folder="f" :vault="vault" :current="current" :open="open" @toggle="(p, o) => emit('toggle', p, o)" />
      </details>
    </li>
    <li v-for="n in folder.notes" :key="n.entry">
      <RouterLink
        :to="`/vaults/${vault}/notes/${n.entry}`"
        :aria-current="n.entry === current ? 'page' : undefined"
        :data-entry="n.entry"
        class="block break-words border-l py-1 pr-2 pl-[1.6875rem] no-underline"
        :class="n.entry === current ? 'border-ink bg-raised font-bold text-ink' : 'border-transparent hover:bg-raised'"
      >{{ noteName(n.path) }}</RouterLink>
    </li>
  </ul>
</template>
