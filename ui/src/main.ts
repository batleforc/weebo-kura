import { createPinia } from "pinia";
import { createApp } from "vue";

import App from "./App.vue";
import { installAuth } from "./api/client";
import "./assets/main.css";
import { loadConfig } from "./config";
import { i18n } from "./i18n";
import { installPrefs } from "./prefs";
import { makeRouter } from "./router";

async function start(): Promise<void> {
  await loadConfig();
  const app = createApp(App).use(createPinia()).use(i18n);
  installAuth();
  installPrefs();
  app.use(makeRouter()).mount("#app");
  if ("serviceWorker" in navigator) {
    void navigator.serviceWorker
      .register("/app/sw.js", { scope: "/app/" })
      .catch(() => undefined);
  }
}

void start();
