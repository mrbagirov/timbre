/* The one place the interface talks to the audio engine.
   This build ships a mock so the window runs in any browser. In Tauri, each method becomes an
   invoke() call to the Rust core (WASAPI capture, the DSP chain, the VB-Cable output):
     detect()      -> invoke("detect_devices")
     openUrl(url)  -> the opener plugin
   The live signal comes from sim.js here; in Tauri, Rust emits levels and the lane history
   at 50 Hz and sim.js is replaced by a listener. */
(function () {
  'use strict';

  function query() {
    var h = location.hash || '', i = h.indexOf('?');
    var q = new URLSearchParams(i >= 0 ? h.slice(i + 1) : '');
    var s = new URLSearchParams(location.search);
    s.forEach(function (v, k) { if (!q.has(k)) q.set(k, v); });
    return q;
  }

  window.Bridge = {
    mock: true,
    detect: function () {
      var q = query();
      return new Promise(function (resolve) {
        setTimeout(function () {
          resolve({
            mic: q.get('mic') === 'none' ? { found: false } : { found: true, name: 'AM8', transport: 'USB' },
            cable: { found: q.get('cable') !== 'none', name: 'CABLE Input' }
          });
        }, 700);
      });
    },
    openUrl: function (url) { window.open(url, '_blank', 'noopener'); },
    query: query
  };
})();
