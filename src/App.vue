<script setup lang="ts">
import { onMounted } from "vue";
import { useIdentityStore } from "./stores/identity";

const identity = useIdentityStore();
onMounted(() => identity.initialize());
</script>

<template>
  <router-view />
</template>
