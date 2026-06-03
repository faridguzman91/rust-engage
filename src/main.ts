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
      darkModeSelector: ".dark",   // we control dark mode by toggling this class on <html>
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
