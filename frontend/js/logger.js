export function createLogger(bridge) {
  return function dbg(message, level = "info") {
    const prefix = [UI] ;
    try {
      if (level === "error") {
        console.error(prefix);
      } else if (level === "warn") {
        console.warn(prefix);
      } else {
        console.log(prefix);
      }
    } catch (error) {
      console.error(error);
    }
    if (bridge) {
      bridge.log(level, message).catch(() => {});
    }
  };
}
