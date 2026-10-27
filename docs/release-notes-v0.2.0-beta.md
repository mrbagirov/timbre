<details open>
<summary><b>English</b></summary>

## Timbre 0.2.0, beta 1

The big change: Timbre now talks to the AM8's own signal processor, so settings work in every program, not only through the virtual cable.

**New**
- **Inside the microphone.** A panel on the main screen controls the AM8 itself: gain (-12 to +12 dB), noise gate, low cut, clarity at 3 kHz and a low shelf at 150 Hz. Every write is read back and checked. These settings live in the microphone's memory, so they survive closing Timbre and are lost when you unplug the USB cable.
- **Live reading.** The headphone knob on the body moves its bar in the window. The analog gain knob cannot be read: the firmware does not see it.
- **Calibration rebuilt.** It records your whisper, normal and loud takes, then plays "before" and "after" on the same recording so you can hear the change. The gain goes into the microphone, the rest stays in the program.
- **One window.** Launching Timbre twice brings the first window forward.
- **Back everywhere.** A back button and Esc on every screen.
- The audio error message now includes the reason given by the driver.

**How it works.** The control channel is the microphone's vendor HID interface (the same MVsilicon protocol that the [fifine-am8](https://github.com/Austrum-lab/fifine-am8) project documented, MIT). Timbre only writes to three known blocks and never touches the firmware, the Feature report or the bootloader commands.

**Known limits.** RGB lighting cannot be controlled over USB on this firmware. The mute button on the body is not tied to the program on purpose. Hardware settings are not restored when you replug the microphone. The installer is not signed, so SmartScreen asks: "More info", then "Run anyway".

**Install.** Run `Timbre_0.2.0_x64-setup.exe` over the old version. You still need [VB-Cable](https://vb-audio.com/Cable/) for Discord, OBS or Zoom, with `CABLE Output` as the microphone.

</details>

<details>
<summary><b>Русский</b></summary>

## Тембр 0.2.0, бета 1

Главное: Тембр теперь работает с собственным процессором AM8, поэтому настройки действуют в любой программе, а не только через виртуальный кабель.

**Новое**
- **В микрофоне.** Панель на главном экране управляет самим AM8: усиление (от −12 до +12 дБ), гейт, срез низов, чёткость на 3 кГц и низкий шельф на 150 Гц. Каждая запись перечитывается и проверяется. Эти настройки живут в памяти микрофона: переживают закрытие Тембра и сбрасываются, если выдернуть USB.
- **Живое чтение.** Ручка наушников на корпусе двигает полоску в окне. Аналоговую ручку усиления прочитать нельзя: прошивка её не видит.
- **Калибровка пересобрана.** Она записывает шёпот, обычную и громкую речь, а в конце проигрывает «было» и «стало» на одной и той же записи, чтобы разницу можно было услышать. Усиление ставится в микрофон, остальное остаётся в программе.
- **Одно окно.** Второй запуск Тембра просто выводит первое окно вперёд.
- **Назад отовсюду.** Кнопка «назад» и Esc на каждом экране.
- В сообщении об ошибке звука теперь есть причина, которую назвал драйвер.

**Как это устроено.** Канал управления — вендорский HID-интерфейс микрофона (тот же протокол MVsilicon, который описал проект [fifine-am8](https://github.com/Austrum-lab/fifine-am8), MIT). Тембр пишет только в три известных блока и не трогает прошивку, Feature-репорт и команды загрузчика.

**Известные ограничения.** Подсветкой RGB по USB на этой прошивке управлять нельзя. Кнопка mute на корпусе специально не связана с программой. Железные настройки не восстанавливаются после переподключения микрофона. Установщик не подписан, поэтому SmartScreen спросит: «Подробнее», затем «Выполнить в любом случае».

**Установка.** Запустите `Timbre_0.2.0_x64-setup.exe` поверх старой версии. Для Discord, OBS и Zoom по-прежнему нужен [VB-Cable](https://vb-audio.com/Cable/), микрофоном выбирается `CABLE Output`.

</details>
