export class TauriBridge {
  constructor() {
    this.invoke = null;
    this.listen = null;
    this.app = null;
  }

  hydrate() {
    const tauri = window.__TAURI__;
    if (!tauri) {
      console.warn("Tauri bridge not available yet");
      return;
    }
    this.invoke = tauri.core?.invoke ?? null;
    this.listen = tauri.event?.listen ?? null;
    this.app = tauri.app ?? null;
  }

  ensure() {
    if (!this.invoke || !this.listen || !this.app) {
      this.hydrate();
    }
    if (!this.invoke || !this.listen || !this.app) {
      throw new Error("Tauri APIs are not available");
    }
    return {
      invoke: this.invoke,
      listen: this.listen,
      app: this.app,
    };
  }

  async call(command, payload) {
    const { invoke } = this.ensure();
    return invoke(command, payload);
  }

  async on(event, handler) {
    const { listen } = this.ensure();
    return listen(event, handler);
  }

  async getVersion() {
    try {
      const { app } = this.ensure();
      if (!app?.getVersion) {
        return null;
      }
      return await app.getVersion();
    } catch (error) {
      console.error(error);
      return null;
    }
  }

  async log(level, message) {
    try {
      await this.call("frontend_log", { level, message });
    } catch (error) {
      console.warn("Failed to forward frontend log", error);
    }
  }
}
