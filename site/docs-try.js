/* API reference interaction: shared playground shop, Try-it panels for anonymous reads and tabbed copyable examples. */
// Every panel sends exactly the method, path and headers the generated reference names, without
// credentials. The shop comes from the reader or site/playground.json; an empty demo value shows the
// "create your shop" path instead of a call that cannot succeed.
const STORE = 'veyndo-docs-playground';

function element(tag, className, key, fallback) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (key) node.dataset.docsCopy = key;
  if (fallback) node.textContent = fallback;
  return node;
}

function remembered() {
  try { return JSON.parse(localStorage.getItem(STORE) || '{}'); } catch { return {}; }
}

function remember(state) {
  try { localStorage.setItem(STORE, JSON.stringify({ origin: state.origin, shop: state.shop })); } catch {}
}

function originOf(value) {
  try {
    const url = new URL(value);
    return (url.protocol === 'https:' || url.protocol === 'http:') ? url.origin : '';
  } catch { return ''; }
}

function field(labelKey, labelText, value, onInput) {
  const label = element('label', 'docs-try-field');
  const caption = element('span', '', labelKey, labelText);
  const input = document.createElement('input');
  input.value = value; input.autocomplete = 'off'; input.spellcheck = false;
  input.addEventListener('input', () => onInput(input.value.trim()));
  label.append(caption, input);
  return { label, input };
}

function configPanel(host, state, config, docsRoot, refresh) {
  host.replaceChildren();
  host.append(element('p', 'docs-try-heading', 'tryConfig', 'Playground shop'));
  const row = element('div', 'docs-try-row');
  const origin = field('tryOrigin', 'API origin', state.origin, value => { state.origin = value; refresh(); });
  const shop = field('tryShop', 'Shop ID', state.shop, value => { state.shop = value; refresh(); });
  shop.input.placeholder = 'my-shop-id';
  row.append(origin.label, shop.label);
  const missing = element('p', 'docs-try-missing');
  missing.append(element('span', '', 'tryNoShop', 'There is no public demo shop yet. Create your own shop, then paste its shop ID here.'), ' ');
  const create = element('a', '', 'tryCreate', 'Create your shop →');
  create.href = config.createShop;
  const guide = element('a', '', 'tryGuide', 'Build your first app →');
  guide.href = new URL('build-your-first-app.html', docsRoot).href;
  missing.append(create, ' ', guide);
  host.append(row, missing, element('p', 'docs-try-note', 'tryAnonymous', 'Sent without credentials, from your browser to the shop.'));
  return { missing, inputs: [origin.input, shop.input] };
}

function pretty(text) {
  try { return JSON.stringify(JSON.parse(text), null, 2); } catch { return text; }
}

function tryPanel(host, state, say) {
  const method = host.dataset.method, template = host.dataset.path;
  const names = JSON.parse(host.dataset.params || '[]');
  const hasBody = host.dataset.body !== undefined;
  host.replaceChildren();
  const header = element('div', 'docs-try-header');
  header.append(element('span', 'docs-try-title', 'tryTitle', 'Try it'));
  const line = element('code', '', '', `${method} ${template}`);
  header.append(line);
  const params = names.map(name => {
    const input = document.createElement('input');
    input.name = name; input.placeholder = name; input.autocomplete = 'off'; input.spellcheck = false;
    const label = element('label', 'docs-try-field');
    label.append(element('span', '', '', name), input);
    return { name, input, label };
  });
  const form = element('form', 'docs-try-form');
  if (params.length) { const row = element('div', 'docs-try-row'); row.append(...params.map(p => p.label)); form.append(row); }
  let body;
  if (hasBody) {
    const label = element('label', 'docs-try-field docs-try-body');
    body = document.createElement('textarea');
    body.rows = 3; body.spellcheck = false;
    body.value = JSON.stringify(JSON.parse(host.dataset.body), null, 2);
    label.append(element('span', '', 'tryBody', 'Request body (JSON)'), body);
    form.append(label);
  }
  const send = element('button', 'docs-try-send', 'trySend', 'Send request');
  send.type = 'submit';
  const status = element('p', 'docs-try-status');
  status.setAttribute('role', 'status');
  const output = element('pre', 'docs-try-output');
  output.hidden = true;
  form.append(send);
  host.append(header, form, status, output);
  form.addEventListener('submit', async event => {
    event.preventDefault();
    output.hidden = true; output.textContent = '';
    const origin = originOf(state.origin);
    if (!origin || !state.shop) { status.textContent = say('tryNoShop', 'There is no public demo shop yet. Create your own shop, then paste its shop ID here.'); return; }
    let path = template;
    for (const p of params) {
      if (!p.input.value.trim()) { status.textContent = say('tryMissing', 'Fill in every path parameter.'); p.input.focus(); return; }
      path = path.replace(`{${p.name}}`, encodeURIComponent(p.input.value.trim()));
    }
    const headers = { accept: 'application/json', 'x-tenant': state.shop };
    let payload;
    if (body) {
      try { payload = JSON.stringify(JSON.parse(body.value || '{}')); } catch { status.textContent = say('tryBadJson', 'The request body is not valid JSON.'); return; }
      headers['content-type'] = 'application/json';
    }
    send.disabled = true; status.textContent = say('tryRunning', 'Calling the shop…');
    try {
      // Anonymous on purpose: no cookies, no stored credentials, nothing beyond the listed headers.
      const response = await fetch(origin + path, { method, headers, body: payload, credentials: 'omit', mode: 'cors', cache: 'no-store' });
      const text = await response.text();
      status.textContent = `HTTP ${response.status} · ${method} ${origin}${path}`;
      output.textContent = pretty(text).slice(0, 20000);
      output.hidden = false;
    } catch {
      status.textContent = say('tryFailed', 'The shop did not answer this browser request. Check the API origin and shop ID.');
    } finally { send.disabled = false; }
  });
}

function copyButton(pre, say) {
  const button = element('button', 'docs-copy-button', 'tryCopy', 'Copy');
  button.type = 'button';
  button.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(pre.textContent);
      button.textContent = say('tryCopied', 'Copied');
      setTimeout(() => { button.textContent = say('tryCopy', 'Copy'); }, 1600);
    } catch { /* Selecting the code by hand still works. */ }
  });
  return button;
}

function exampleTabs(host, index, say) {
  const labels = [...host.querySelectorAll(':scope > .docs-example-label')];
  const blocks = labels.map(label => label.nextElementSibling).filter(node => node?.tagName === 'PRE');
  if (blocks.length !== labels.length || !labels.length) return;
  const list = element('div', 'docs-example-tabs');
  list.setAttribute('role', 'tablist');
  const tabs = labels.map((label, i) => {
    const tab = element('button', '', '', label.textContent);
    tab.type = 'button'; tab.id = `docs-example-${index}-${i}`;
    tab.setAttribute('role', 'tab');
    blocks[i].setAttribute('role', 'tabpanel');
    blocks[i].setAttribute('aria-labelledby', tab.id);
    label.remove();
    return tab;
  });
  const select = chosen => {
    tabs.forEach((tab, i) => {
      const active = i === chosen;
      tab.setAttribute('aria-selected', String(active));
      tab.tabIndex = active ? 0 : -1;
      blocks[i].hidden = !active;
    });
  };
  tabs.forEach((tab, i) => {
    tab.addEventListener('click', () => select(i));
    tab.addEventListener('keydown', event => {
      const step = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0;
      if (step) { const next = (i + step + tabs.length) % tabs.length; select(next); tabs[next].focus(); }
    });
  });
  list.append(...tabs);
  const tools = element('div', 'docs-example-bar');
  tools.append(list, copyButton({ get textContent() { return blocks.find(b => !b.hidden)?.textContent ?? ''; } }, say));
  host.prepend(tools);
  select(0);
}

export async function renderTry(assets, docsRoot, say) {
  const panels = [...document.querySelectorAll('.docs-try')];
  const configs = [...document.querySelectorAll('.docs-try-config')];
  [...document.querySelectorAll('.docs-examples')].forEach((host, index) => exampleTabs(host, index, say));
  if (!panels.length && !configs.length) return;
  let config = { apiBase: 'https://api.veyndo.ai', demoShop: '', createShop: 'https://start.veyndo.ai/start' };
  try {
    const response = await fetch(new URL('playground.json', assets));
    if (response.ok) config = { ...config, ...(await response.json()) };
  } catch { /* The built-in defaults keep the panels usable. */ }
  const saved = remembered(), query = new URLSearchParams(location.search);
  const state = { origin: query.get('origin') || saved.origin || config.apiBase, shop: query.get('shop') || saved.shop || config.demoShop };
  const views = [];
  const refresh = () => {
    remember(state);
    const ready = Boolean(originOf(state.origin) && state.shop);
    for (const view of views) {
      view.missing.hidden = ready;
      view.inputs[0].value === state.origin || (view.inputs[0].value = state.origin);
      view.inputs[1].value === state.shop || (view.inputs[1].value = state.shop);
    }
  };
  configs.forEach(host => views.push(configPanel(host, state, config, docsRoot, refresh)));
  panels.forEach(host => tryPanel(host, state, say));
  refresh();
}
