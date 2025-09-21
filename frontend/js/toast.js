export class ToastController {
  constructor(element) {
    this.element = element;
    this.timer = null;
  }

  show(message, type = "info") {
    if (!this.element) {
      return;
    }
    this.element.textContent = message;
    this.element.dataset.type = type;
    this.element.hidden = false;
    clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      this.element.hidden = true;
    }, 2800);
  }
}
