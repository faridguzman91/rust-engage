// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

// @faridguzman91: App entry point — wires PrimeVue 4 (Aura dark theme), Pinia, and Vue Router.
// Dark mode is class-based (.dark on <html>) so PrimeVue tokens respond correctly.
import { createApp } from "vue";
import { createPinia } from "pinia";
import PrimeVue from "primevue/config";
import ToastService from "primevue/toastservice";
import Tooltip from "primevue/tooltip";
import Aura from "@primeuix/themes/aura";
import router from "./router";
import App from "./App.vue";
import "primeicons/primeicons.css";
import "./styles/global.css";

const app = createApp(App);

app.use(createPinia());
app.use(router);
app.use(PrimeVue, {
  theme: {
    preset: Aura,
    options: {
      // @faridguzman91: we toggle dark mode ourselves by adding .dark to <html> on mount
      darkModeSelector: ".dark",
      cssLayer: {
        name: "primevue",
        order: "tailwind-base, primevue, tailwind-utilities",
      },
    },
  },
});
app.use(ToastService);
app.directive("tooltip", Tooltip);

app.mount("#app");
