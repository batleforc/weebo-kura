<script setup lang="ts">
import type { Folder } from "./tree";
import { noteName } from "./tree";

defineProps<{ folder: Folder; vault: string; current?: string }>();
</script>

<template>
  <ul class="pl-3">
    <li v-for="f in folder.folders" :key="f.path">
      <details open>
        <summary>{{ f.name }}</summary>
        <FolderTree :folder="f" :vault="vault" :current="current" />
      </details>
    </li>
    <li v-for="n in folder.notes" :key="n.entry">
      <RouterLink
        :to="`/vaults/${vault}/notes/${n.entry}`"
        :aria-current="n.entry === current ? 'page' : undefined"
        :class="n.entry === current ? 'underline' : ''"
      >
        {{ noteName(n.path) }}
      </RouterLink>
    </li>
  </ul>
</template>
