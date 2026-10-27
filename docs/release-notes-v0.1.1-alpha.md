<details open>
<summary><b>English</b></summary>

## Timbre 0.1.1 alpha

Changes since 0.1.0, from the first test on a real AM8.

**Fixes**
- The first calibration step no longer asks you to overload the microphone. The AM8 is a quiet dynamic microphone, so Timbre now adds the missing level itself with a software gain of up to +36 dB. The wizard calculates it, and the first step only checks that nothing clips.

**New**
- **Voices.** A new "character" lane with pitch, warmth, thickness, a radio band and a robot effect, and seven voice presets: warm broadcast, deep, bright podcast, thick, radio, robot, high.
- **Presets in a drop-down** with two groups, tasks and voices. The tray has a Voices submenu.
- **Software gain** slider under the input meter.
- **Diagnostics.** Settings, "copy the report": audio devices with their formats and a read-only look at the USB control interfaces of the AM8. Nothing is written to the microphone.

**Known limits.** The pitch shifter is a simple granular one: fine for about ±5 semitones. The automatic EQ targets are still a first guess.

**Install.** Run `Timbre_0.1.1_x64-setup.exe` over the old version, settings are kept. The installer is not signed, so SmartScreen asks: "More info", then "Run anyway". You still need [VB-Cable](https://vb-audio.com/Cable/), and `CABLE Output` as the microphone in Discord, OBS or Zoom.

</details>

<details>
<summary><b>Русский</b></summary>

## Тембр 0.1.1, альфа

Что изменилось с 0.1.0 после первой обкатки на настоящем AM8.

**Исправлено**
- Первый шаг калибровки больше не требует перегружать микрофон. AM8 динамический и тихий, поэтому Тембр теперь сам добавляет недостающий уровень программным усилением до +36 дБ. Размер усиления считает мастер, а первый шаг только проверяет, что нет перегруза.

**Новое**
- **Голоса.** Новая дорожка «характер» с высотой, теплом, объёмом, радио-полосой и роботом, и семь голосовых пресетов: тёплый эфир, глубокий, яркий подкаст, объёмный, радио, робот, высокий.
- **Пресеты в выпадающем списке** с двумя группами: для задач и голоса. В трее есть подменю «Голоса».
- **Ползунок программного усиления** под входным метром.
- **Диагностика.** Настройки, «скопировать отчёт»: звуковые устройства с форматами и просмотр USB-интерфейсов управления AM8 только для чтения. В микрофон ничего не записывается.

**Известные ограничения.** Сдвиг высоты простой, гранулярный: хорош примерно до ±5 полутонов. Целевые значения автоматического эквалайзера всё ещё прикидочные.

**Установка.** Запустите `Timbre_0.1.1_x64-setup.exe` поверх старой версии, настройки сохранятся. Установщик без подписи, поэтому SmartScreen спросит: «Подробнее», «Выполнить в любом случае». Нужен [VB-Cable](https://vb-audio.com/Cable/), а в Discord, OBS или Zoom микрофоном выбирайте `CABLE Output`.

</details>
