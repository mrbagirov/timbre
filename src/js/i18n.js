/* Localization. The Russian text in the sources is the key; the English dictionary (i18n.en.js) maps it to English.
   - exact strings are looked up as they are;
   - strings with numbers or names use templates with {1}, {2} holes, matched by pattern;
   - the page language is fixed per load: the switch stores the choice and reloads. */
(function () {
  'use strict';

  var KEY = 'timbre.lang';
  var lang = 'ru';
  try {
    var saved = localStorage.getItem(KEY);
    lang = saved === 'ru' || saved === 'en' ? saved : /^ru/i.test(navigator.language || '') ? 'ru' : 'en';
  } catch (e) { /* storage may be unavailable */ }

  var dict = window.I18N_EN || {};
  var exact = Object.create(null), templates = [];
  Object.keys(dict).forEach(function (k) {
    if (k.indexOf('{}') < 0) { exact[k] = dict[k]; return; }
    var parts = k.split('{}'), n = parts.length - 1;
    var re = new RegExp('^' + parts.map(function (p) { return p.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'); }).join('(.+?)') + '$');
    templates.push({ re: re, n: n, en: dict[k], weight: parts.join('').length });
  });
  templates.sort(function (a, b) { return b.weight - a.weight; }); // the most specific template first

  var DEC = lang === 'en' ? '.' : ',';
  function number(s) { return /^[−+-]?\d+[.,]\d+$/.test(s) ? s.replace(/[.,]/, DEC) : s; }

  function t(s) {
    if (lang !== 'en' || typeof s !== 'string') return s;
    if (exact[s] !== undefined) return exact[s];
    for (var i = 0; i < templates.length; i++) {
      var m = templates[i].re.exec(s);
      if (m) return templates[i].en.replace(/\{(\d)\}/g, function (x, d) { var c = m[+d]; return exact[c] !== undefined ? exact[c] : number(c); });
    }
    return s;
  }

  /* "{}" holes filled in order, for strings built from numbers */
  function f(tpl) { var a = Array.prototype.slice.call(arguments, 1), i = 0; return tpl.replace(/\{\}/g, function () { return a[i++]; }); }

  var originals = new WeakMap();
  var ATTRS = ['aria-label', 'title', 'alt', 'placeholder', 'label'];

  function apply(root) {
    if (lang !== 'en') return;
    var walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode: function (n) {
        var p = n.parentNode;
        if (!p || p.nodeName === 'SCRIPT' || p.nodeName === 'STYLE' || p.closest('[data-no-i18n]')) return NodeFilter.FILTER_REJECT;
        return /\S/.test(n.nodeValue) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
      }
    });
    var nodes = [], n;
    while ((n = walker.nextNode())) nodes.push(n);
    nodes.forEach(function (node) {
      var o = originals.has(node) ? originals.get(node) : node.nodeValue;
      originals.set(node, o);
      var core = o.trim(), out = t(core.replace(/\s+/g, ' '));
      if (out !== core) node.nodeValue = o.replace(core, out);
    });
    root.querySelectorAll(ATTRS.map(function (a) { return '[' + a + ']'; }).join(',')).forEach(function (el) {
      if (el.closest('[data-no-i18n]')) return;
      ATTRS.forEach(function (a) { if (el.hasAttribute(a)) { var v = el.getAttribute(a), tv = t(v); if (tv !== v) el.setAttribute(a, tv); } });
    });
  }

  function set(l) {
    try { localStorage.setItem(KEY, l); } catch (e) { /* ignore */ }
    location.reload();
  }

  document.documentElement.lang = lang;
  if (lang === 'en') {
    document.title = t(document.title);
    var meta = document.querySelector('meta[name="description"]');
    if (meta) meta.setAttribute('content', t(meta.getAttribute('content')));
  }

  window.I18N = { t: t, f: f, lang: lang, dec: DEC, apply: apply, set: set };
  apply(document.body);
})();
