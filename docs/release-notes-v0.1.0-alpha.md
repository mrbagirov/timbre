<details open>
<summary><b>English</b></summary>

## Timbre 0.1.0 alpha

Voice processing for the Fifine AM8 microphone on Windows: gate, noise suppression (RNNoise), EQ, compressor and limiter in one window, with presets and a calibration wizard. The processed sound goes to Discord, OBS and Zoom through VB-Cable.

**What is in this build**
- A processing chain with a live lane for every step
- Presets: beginner, stream, voice-over, call, and your own "mine"
- A six-step calibration wizard with explained results and a before/after switch
- Tray icon, autostart, a global mute hotkey, one-click bypass
- English and Russian interface, switched in the settings

**Install**
1. Install [VB-Cable](https://vb-audio.com/Cable/) and restart.
2. Run `Timbre_0.1.0_x64-setup.exe`. No administrator rights needed. SmartScreen warns about an unknown publisher because the installer is not signed yet: "More info", then "Run anyway".
3. In Discord, OBS or Zoom pick `CABLE Output` as the microphone, then press the big button in Timbre.

**Alpha notes.** Nobody has listened to a live AM8 through this build yet. Capture and cable output may fail on unusual device formats, noise suppression is only checked on synthetic input, and the automatic EQ targets are a first guess. If something breaks, attach the error text from the settings.

</details>

<details>
<summary><b>Русский</b></summary>

## Тембр 0.1.0, альфа

Обработка голоса для микрофона Fifine AM8 на Windows: гейт, шумодав (RNNoise), эквалайзер, компрессор и лимитер в одном окне, с пресетами и мастером калибровки. Обработанный звук уходит в Discord, OBS и Zoom через VB-Cable.

**Что в этой сборке**
- Цепочка обработки с живой дорожкой у каждого шага
- Пресеты: новичок, стрим, закадр, созвон и ваш собственный «мой»
- Мастер калибровки из шести шагов с объяснёнными результатами и переключателем «было / стало»
- Иконка в трее, автозапуск, глобальная горячая клавиша мьюта, обход обработки одним нажатием
- Интерфейс на русском и английском, язык переключается в настройках

**Установка**
1. Поставьте [VB-Cable](https://vb-audio.com/Cable/) и перезагрузите компьютер.
2. Запустите `Timbre_0.1.0_x64-setup.exe`. Права администратора не нужны. SmartScreen предупредит о неизвестном издателе, потому что установщик пока без подписи: «Подробнее», «Выполнить в любом случае».
3. В Discord, OBS или Zoom выберите микрофоном `CABLE Output`, затем нажмите большую кнопку в тембре.

**Об альфе.** Живой AM8 через эту сборку пока никто не слушал. Захват и вывод в кабель могут не открыться на нестандартных форматах устройства, подавление шума проверено только на синтетике, а целевые значения автоматического эквалайзера прикидочные. Если что-то сломалось, приложите текст ошибки из настроек.

</details>
