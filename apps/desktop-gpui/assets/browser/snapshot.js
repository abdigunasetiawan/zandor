// Spike S-B3: snapshot aksesibilitas ringkas + ref. Disuntik lewat with_initialization_script.
// Ref disimpan di closure (bukan properti global yang bisa ditimpa halaman) dan API dikunci (non-writable).
(() => {
  if (window.__zsnap) return;
  let refs = new Map(); // "e1" -> WeakRef(elemen)
  let n = 0;
  const INTERACTIVE = 'a[href],button,input,select,textarea,summary,[role=button],[role=link],[role=checkbox],[role=textbox],[role=menuitem],[contenteditable=""],[contenteditable=true],[tabindex]:not([tabindex="-1"])';
  const visible = (el) => {
    const r = el.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) return false;
    const cs = getComputedStyle(el);
    return cs.visibility !== 'hidden' && cs.display !== 'none' && cs.opacity !== '0';
  };
  const roleOf = (el) => {
    const r = el.getAttribute('role');
    if (r) return r;
    const t = el.tagName.toLowerCase();
    if (t === 'a') return 'link';
    if (t === 'button' || t === 'summary') return 'button';
    if (t === 'select') return 'combobox';
    if (t === 'textarea') return 'textbox';
    if (t === 'input') {
      const ty = (el.getAttribute('type') || 'text').toLowerCase();
      return ({ checkbox: 'checkbox', radio: 'radio', submit: 'button', button: 'button', range: 'slider' })[ty] || 'textbox';
    }
    return 'generic';
  };
  const nameOf = (el) => {
    const v = el.getAttribute('aria-label') || (el.labels && el.labels[0] && el.labels[0].innerText) ||
      el.getAttribute('placeholder') || el.getAttribute('title') || el.getAttribute('alt') ||
      (el.tagName === 'INPUT' && ['submit', 'button'].includes(el.type) ? el.value : '') || el.innerText || '';
    return String(v).replace(/\s+/g, ' ').trim().slice(0, 80);
  };
  const SENSITIVE = (el) => el.tagName === 'INPUT' && ['password'].includes((el.type || '').toLowerCase()) || /otp|cc-|card|cvv|ssn/i.test((el.autocomplete || '') + (el.name || '') + (el.id || ''));
  const snap = () => {
    refs = new Map(); n = 0;
    const lines = [];
    for (const el of document.querySelectorAll(INTERACTIVE)) {
      if (!visible(el)) continue;
      const id = 'e' + (++n);
      refs.set(id, new WeakRef(el));
      const role = roleOf(el);
      let line = `[${id}] ${role} "${nameOf(el)}"`;
      if (role === 'textbox' || role === 'combobox') line += SENSITIVE(el) ? ' value=<disamarkan>' : ` value="${String(el.value || '').slice(0, 60)}"`;
      if (role === 'checkbox' || role === 'radio') line += el.checked ? ' checked' : '';
      if (role === 'link') { try { line += ' -> ' + new URL(el.href, location.href).pathname; } catch (_) {} }
      lines.push(line);
      if (n >= 200) { lines.push('... (dipotong)'); break; }
    }
    const text = (document.body ? document.body.innerText : '').replace(/\s+/g, ' ').trim().slice(0, 2000);
    return JSON.stringify({ url: location.href, title: document.title, refs: n, tree: lines.join('\n'), text });
  };
  const rect = (id) => {
    const el = refs.get(id)?.deref();
    if (!el || !el.isConnected) return JSON.stringify({ error: 'ref basi' });
    el.scrollIntoView({ block: 'center', inline: 'center' });
    const r = el.getBoundingClientRect();
    const x = r.left + r.width / 2, y = r.top + r.height / 2;
    const top = document.elementFromPoint(x, y);
    const hit = top === el || el.contains(top) || (top && top.contains(el) && false);
    return JSON.stringify({ x, y, hit, covered: hit ? '' : (top ? top.tagName : 'none') });
  };
  const focus = (id) => { const el = refs.get(id)?.deref(); if (!el) return 'ref basi'; el.focus(); return 'ok'; };
  const api = Object.freeze({ snap, rect, focus });
  Object.defineProperty(window, '__zsnap', { value: api.snap, writable: false, configurable: false });
  Object.defineProperty(window, '__zrect', { value: api.rect, writable: false, configurable: false });
  Object.defineProperty(window, '__zfocus', { value: api.focus, writable: false, configurable: false });
})();
