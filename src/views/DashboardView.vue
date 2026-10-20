<script setup lang="ts">
import PageHeader from "../components/PageHeader.vue";
import { formatDate } from "../utils/format";

const today = new Date();
const isoToday = [
  today.getFullYear(),
  String(today.getMonth() + 1).padStart(2, "0"),
  String(today.getDate()).padStart(2, "0"),
].join("-");
</script>

<template>
  <PageHeader title="Tableau de bord" :subtitle="formatDate(isoToday, 'long')" />
  <p class="placeholder">Tableau de bord (V1-07.4).</p>
</template>
