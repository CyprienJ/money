<script setup lang="ts">
import { onMounted, ref } from "vue";
import { commands } from "./bindings";

const version = ref("");

onMounted(async () => {
  version.value = await commands.appVersion();
});
</script>

<template>
  <main>
    <h1>Finances perso</h1>
    <p v-if="version">Version {{ version }}</p>
  </main>
</template>
