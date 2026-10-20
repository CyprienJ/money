<script setup lang="ts">
import PageHeader from "../components/PageHeader.vue";
import { formatDate, todayIso } from "../utils/format";
</script>

<template>
  <PageHeader title="Tableau de bord" :subtitle="formatDate(todayIso(), 'long')" />
  <p class="placeholder">Tableau de bord (V1-07.4).</p>
</template>
