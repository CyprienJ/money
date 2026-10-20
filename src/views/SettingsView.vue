<script setup lang="ts">
import { onMounted, ref } from "vue";
import { commands } from "../bindings";
import PageHeader from "../components/PageHeader.vue";

const version = ref("");

onMounted(async () => {
  version.value = await commands.appVersion();
});
</script>

<template>
  <PageHeader title="Paramètres" />
  <p class="placeholder">Paramètres (V1-07.5).</p>
  <p v-if="version" class="placeholder">Version {{ version }}</p>
</template>
