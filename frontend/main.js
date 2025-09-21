import { AppController } from "./js/app.js";

document.addEventListener("DOMContentLoaded", () => {
  const app = new AppController();
  app.start().catch((error) => console.error(error));
});
