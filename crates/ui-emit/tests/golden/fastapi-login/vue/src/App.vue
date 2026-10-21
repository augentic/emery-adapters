<!-- Generated from design-system/ — do not edit manually. -->
<script setup lang="ts">
import { computed, ref } from "vue";
import LoginScreen from "./views/LoginScreen.vue";

const screens = {
  "login": LoginScreen,
} as const;

const route = ref(window.location.hash.slice(1) || "login");
window.addEventListener("hashchange", () => {
  route.value = window.location.hash.slice(1) || "login";
});
const current = computed(() => screens[route.value as keyof typeof screens] ?? screens["login"]);
</script>

<template>
  <component :is="current" />
</template>
