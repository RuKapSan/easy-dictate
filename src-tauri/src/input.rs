use std::sync::Mutex;

use anyhow::{anyhow, Result};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

#[derive(Default)]
pub struct KeyboardController {
    inner: Mutex<Option<Enigo>>,
    settings: Settings,
}

impl KeyboardController {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: Mutex::new(None),
            settings: Settings::default(),
        })
    }

    pub fn type_text(&self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        let mut guard = self
            .inner
            .lock()
            .map_err(|_| anyhow!("Не удалось захватить эмулятор клавиатуры"))?;
        if guard.is_none() {
            *guard = Some(
                Enigo::new(&self.settings)
                    .map_err(|e| anyhow!("Ошибка инициализации эмулятора: {e}"))?,
            );
        }
        if let Some(enigo) = guard.as_mut() {
            enigo
                .text(text)
                .map_err(|e| anyhow!("Не удалось ввести текст: {e}"))?
        } else {
            return Err(anyhow!("Эмулятор клавиатуры не инициализирован"));
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn paste(&self) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| anyhow!("Не удалось захватить эмулятор клавиатуры"))?;
        if guard.is_none() {
            *guard = Some(
                Enigo::new(&self.settings)
                    .map_err(|e| anyhow!("Ошибка инициализации эмулятора: {e}"))?,
            );
        }
        if let Some(enigo) = guard.as_mut() {
            enigo
                .key(Key::Control, Direction::Press)
                .map_err(|e| anyhow!("Не удалось нажать Ctrl: {e}"))?;
            enigo
                .key(Key::Unicode('v'), Direction::Click)
                .map_err(|e| anyhow!("Не удалось нажать V: {e}"))?;
            enigo
                .key(Key::Control, Direction::Release)
                .map_err(|e| anyhow!("Не удалось отпустить Ctrl: {e}"))?;
        } else {
            return Err(anyhow!("Эмулятор клавиатуры не инициализирован"));
        }
        Ok(())
    }
}
