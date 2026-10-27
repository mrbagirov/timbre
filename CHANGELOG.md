# Changelog

## 0.2.0 (beta 1)
- Controls inside the AM8 itself over its vendor USB channel: gain, noise gate, low cut, clarity and low shelf. Every write is checked by reading it back; settings live in the microphone's RAM.
- Live reading from the microphone: the headphone knob is shown in the window.
- Calibration rebuilt: it records your real takes, plays "before" and "after" on the same recording, and puts the gain into the microphone.
- Only one window can run at a time; a second launch brings the first one forward.
- Back button and Esc on every screen.
- The audio error now says what the driver reported.

## 0.1.1 (alpha)
- Software input gain, calculated by the calibration; the first wizard step only checks for clipping.
- Voice character lane (pitch, warmth, thickness, radio, robot) and seven voice presets.
- Presets in a grouped drop-down; Voices submenu in the tray.
- Diagnostics report with a read-only look at the microphone's USB control interfaces.
- 18 DSP tests.

## 0.1.0 (alpha)
- First release: gate, noise suppression (RNNoise), EQ, compressor, limiter, presets, calibration wizard, tray, hotkey, autostart, English and Russian interface.
