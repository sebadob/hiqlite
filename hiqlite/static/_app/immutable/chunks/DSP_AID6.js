var __typeError = (msg) => {
  throw TypeError(msg);
};
var __accessCheck = (obj, member, msg) => member.has(obj) || __typeError("Cannot " + msg);
var __privateGet = (obj, member, getter) => (__accessCheck(obj, member, "read from private field"), getter ? getter.call(obj) : member.get(obj));
var __privateAdd = (obj, member, value) => member.has(obj) ? __typeError("Cannot add the same private member more than once") : member instanceof WeakSet ? member.add(obj) : member.set(obj, value);
var _a, _b, _e2, _t2, _n, _a2, _r, _o, _s, _i, _c, _e3, _d, _e4, _e5;
import { b9 as be, a7 as Ne, a8 as U, g as T, T as I, a9 as Z, bf as De, bg as pt } from "./TbIIo73h.js";
class ke {
  constructor(t, n) {
    this.status = t, typeof n == "string" ? this.body = { message: n } : n ? this.body = n : this.body = { message: `Error: ${t}` };
  }
  toString() {
    return JSON.stringify(this.body);
  }
}
class Se {
  constructor(t, n) {
    this.status = t, this.location = n;
  }
}
class Ee extends Error {
  constructor(t, n, a) {
    super(a), this.status = t, this.text = n;
  }
}
new URL("sveltekit-internal://");
function gt(e, t) {
  return e === "/" || t === "ignore" ? e : t === "never" ? e.endsWith("/") ? e.slice(0, -1) : e : t === "always" && !e.endsWith("/") ? e + "/" : e;
}
function _t(e) {
  return e.split("%25").map(decodeURI).join("%25");
}
function mt(e) {
  for (const t in e) e[t] = decodeURIComponent(e[t]);
  return e;
}
function de({ href: e }) {
  return e.split("#")[0];
}
function wt(...e) {
  let t = 5381;
  for (const n of e) if (typeof n == "string") {
    let a = n.length;
    for (; a; ) t = t * 33 ^ n.charCodeAt(--a);
  } else if (ArrayBuffer.isView(n)) {
    const a = new Uint8Array(n.buffer, n.byteOffset, n.byteLength);
    let r = a.length;
    for (; r; ) t = t * 33 ^ a[--r];
  } else throw new TypeError("value must be a string or TypedArray");
  return (t >>> 0).toString(36);
}
new TextEncoder();
new TextDecoder();
function vt(e) {
  const t = atob(e), n = new Uint8Array(t.length);
  for (let a = 0; a < t.length; a++) n[a] = t.charCodeAt(a);
  return n;
}
const yt = window.fetch;
window.fetch = (e, t) => ((e instanceof Request ? e.method : (t == null ? void 0 : t.method) || "GET") !== "GET" && F.delete(Re(e)), yt(e, t));
const F = /* @__PURE__ */ new Map();
function bt(e, t) {
  const n = Re(e, t), a = document.querySelector(n);
  if (a == null ? void 0 : a.textContent) {
    a.remove();
    let { body: r, ...s } = JSON.parse(a.textContent);
    const o = a.getAttribute("data-ttl");
    return o && F.set(n, { body: r, init: s, ttl: 1e3 * Number(o) }), a.getAttribute("data-b64") !== null && (r = vt(r)), Promise.resolve(new Response(r, s));
  }
  return window.fetch(e, t);
}
function kt(e, t, n) {
  if (F.size > 0) {
    const a = Re(e, n), r = F.get(a);
    if (r) {
      if (performance.now() < r.ttl && ["default", "force-cache", "only-if-cached", void 0].includes(n == null ? void 0 : n.cache)) return new Response(r.body, r.init);
      F.delete(a);
    }
  }
  return window.fetch(t, n);
}
function Re(e, t) {
  let a = `script[data-sveltekit-fetched][data-url=${JSON.stringify(e instanceof Request ? e.url : e)}]`;
  if ((t == null ? void 0 : t.headers) || (t == null ? void 0 : t.body)) {
    const r = [];
    t.headers && r.push([...new Headers(t.headers)].join(",")), t.body && (typeof t.body == "string" || ArrayBuffer.isView(t.body)) && r.push(t.body), a += `[data-hash="${wt(...r)}"]`;
  }
  return a;
}
const St = /^(\[)?(\.\.\.)?(\w+)(?:=(\w+))?(\])?$/;
function Et(e) {
  const t = [];
  return { pattern: e === "/" ? /^\/$/ : new RegExp(`^${xt(e).map((a) => {
    const r = /^\[\.\.\.(\w+)(?:=(\w+))?\]$/.exec(a);
    if (r) return t.push({ name: r[1], matcher: r[2], optional: false, rest: true, chained: true }), "(?:/([^]*))?";
    const s = /^\[\[(\w+)(?:=(\w+))?\]\]$/.exec(a);
    if (s) return t.push({ name: s[1], matcher: s[2], optional: true, rest: false, chained: true }), "(?:/([^/]+))?";
    if (!a) return;
    const o = a.split(/\[(.+?)\](?!\])/);
    return "/" + o.map((c, l) => {
      if (l % 2) {
        if (c.startsWith("x+")) return he(String.fromCharCode(parseInt(c.slice(2), 16)));
        if (c.startsWith("u+")) return he(String.fromCharCode(...c.slice(2).split("-").map((m) => parseInt(m, 16))));
        const f = St.exec(c), [, h, w, u, g] = f;
        return t.push({ name: u, matcher: g, optional: !!h, rest: !!w, chained: w ? l === 1 && o[0] === "" : false }), w ? "([^]*?)" : h ? "([^/]*)?" : "([^/]+?)";
      }
      return he(c);
    }).join("");
  }).join("")}/?$`), params: t };
}
function Rt(e) {
  return e !== "" && !/^\([^)]+\)$/.test(e);
}
function xt(e) {
  return e.slice(1).split("/").filter(Rt);
}
function At(e, t, n) {
  const a = {}, r = e.slice(1), s = r.filter((i) => i !== void 0);
  let o = 0;
  for (let i = 0; i < t.length; i += 1) {
    const c = t[i];
    let l = r[i - o];
    if (c.chained && c.rest && o && (l = r.slice(i - o, i + 1).filter((f) => f).join("/"), o = 0), l === void 0) {
      c.rest && (a[c.name] = "");
      continue;
    }
    if (!c.matcher || n[c.matcher](l)) {
      a[c.name] = l;
      const f = t[i + 1], h = r[i + 1];
      f && !f.rest && f.optional && h && c.chained && (o = 0), !f && !h && Object.keys(a).length === s.length && (o = 0);
      continue;
    }
    if (c.optional && c.chained) {
      o++;
      continue;
    }
    return;
  }
  if (!o) return a;
}
function he(e) {
  return e.normalize().replace(/[[\]]/g, "\\$&").replace(/%/g, "%25").replace(/\//g, "%2[Ff]").replace(/\?/g, "%3[Ff]").replace(/#/g, "%23").replace(/[.*+?^${}()|\\]/g, "\\$&");
}
function Lt({ nodes: e, server_loads: t, dictionary: n, matchers: a }) {
  const r = new Set(t);
  return Object.entries(n).map(([i, [c, l, f]]) => {
    const { pattern: h, params: w } = Et(i), u = { id: i, exec: (g) => {
      const m = h.exec(g);
      if (m) return At(m, w, a);
    }, errors: [1, ...f || []].map((g) => e[g]), layouts: [0, ...l || []].map(o), leaf: s(c) };
    return u.errors.length = u.layouts.length = Math.max(u.errors.length, u.layouts.length), u;
  });
  function s(i) {
    const c = i < 0;
    return c && (i = ~i), [c, e[i]];
  }
  function o(i) {
    return i === void 0 ? i : [r.has(i), e[i]];
  }
}
function We(e, t = JSON.parse) {
  try {
    return t(sessionStorage[e]);
  } catch {
  }
}
function qe(e, t, n = JSON.stringify) {
  const a = n(t);
  try {
    sessionStorage[e] = a;
  } catch {
  }
}
const A = ((_a = globalThis.__sveltekit_uib792) == null ? void 0 : _a.base) ?? "/dashboard", Ut = ((_b = globalThis.__sveltekit_uib792) == null ? void 0 : _b.assets) ?? A ?? "", Tt = "1765301781659", Ye = "sveltekit:snapshot", ze = "sveltekit:scroll", He = "sveltekit:states", It = "sveltekit:pageurl", B = "sveltekit:history", W = "sveltekit:navigation", j = { tap: 1, hover: 2, viewport: 3, eager: 4, off: -1, false: -1 }, xe = location.origin;
function Je(e) {
  if (e instanceof URL) return e;
  let t = document.baseURI;
  if (!t) {
    const n = document.getElementsByTagName("base");
    t = n.length ? n[0].href : document.URL;
  }
  return new URL(e, t);
}
function ce() {
  return { x: pageXOffset, y: pageYOffset };
}
function V(e, t) {
  return e.getAttribute(`data-sveltekit-${t}`);
}
const Ve = { ...j, "": j.hover };
function Xe(e) {
  let t = e.assignedSlot ?? e.parentNode;
  return (t == null ? void 0 : t.nodeType) === 11 && (t = t.host), t;
}
function Qe(e, t) {
  for (; e && e !== t; ) {
    if (e.nodeName.toUpperCase() === "A" && e.hasAttribute("href")) return e;
    e = Xe(e);
  }
}
function _e(e, t, n) {
  let a;
  try {
    if (a = new URL(e instanceof SVGAElement ? e.href.baseVal : e.href, document.baseURI), n && a.hash.match(/^#[^/]/)) {
      const i = location.hash.split("#")[1] || "/";
      a.hash = `#${i}${a.hash}`;
    }
  } catch {
  }
  const r = e instanceof SVGAElement ? e.target.baseVal : e.target, s = !a || !!r || le(a, t, n) || (e.getAttribute("rel") || "").split(/\s+/).includes("external"), o = (a == null ? void 0 : a.origin) === xe && e.hasAttribute("download");
  return { url: a, external: s, target: r, download: o };
}
function ee(e) {
  let t = null, n = null, a = null, r = null, s = null, o = null, i = e;
  for (; i && i !== document.documentElement; ) a === null && (a = V(i, "preload-code")), r === null && (r = V(i, "preload-data")), t === null && (t = V(i, "keepfocus")), n === null && (n = V(i, "noscroll")), s === null && (s = V(i, "reload")), o === null && (o = V(i, "replacestate")), i = Xe(i);
  function c(l) {
    switch (l) {
      case "":
      case "true":
        return true;
      case "off":
      case "false":
        return false;
      default:
        return;
    }
  }
  return { preload_code: Ve[a ?? "off"], preload_data: Ve[r ?? "off"], keepfocus: c(t), noscroll: c(n), reload: c(s), replace_state: c(o) };
}
function Be(e) {
  const t = be(e);
  let n = true;
  function a() {
    n = true, t.update((o) => o);
  }
  function r(o) {
    n = false, t.set(o);
  }
  function s(o) {
    let i;
    return t.subscribe((c) => {
      (i === void 0 || n && c !== i) && o(i = c);
    });
  }
  return { notify: a, set: r, subscribe: s };
}
const Ze = { v: () => {
} };
function Ot() {
  const { set: e, subscribe: t } = be(false);
  let n;
  async function a() {
    clearTimeout(n);
    try {
      const r = await fetch(`${Ut}/_app/version.json`, { headers: { pragma: "no-cache", "cache-control": "no-cache" } });
      if (!r.ok) return false;
      const o = (await r.json()).version !== Tt;
      return o && (e(true), Ze.v(), clearTimeout(n)), o;
    } catch {
      return false;
    }
  }
  return { subscribe: t, check: a };
}
function le(e, t, n) {
  return e.origin !== xe || !e.pathname.startsWith(t) ? true : n ? e.pathname !== location.pathname : false;
}
function rn(e) {
}
const et = /* @__PURE__ */ new Set(["load", "prerender", "csr", "ssr", "trailingSlash", "config"]);
[...et];
const Pt = /* @__PURE__ */ new Set([...et]);
[...Pt];
function $t(e) {
  return e.filter((t) => t != null);
}
function Ae(e) {
  return e instanceof ke || e instanceof Ee ? e.status : 500;
}
function Ct(e) {
  return e instanceof Ee ? e.text : "Internal Error";
}
let k, Y, pe;
const jt = Ne.toString().includes("$$") || /function \w+\(\) \{\}/.test(Ne.toString());
jt ? (k = { data: {}, form: null, error: null, params: {}, route: { id: null }, state: {}, status: -1, url: new URL("https://example.com") }, Y = { current: null }, pe = { current: false }) : (k = new (_c = class {
  constructor() {
    __privateAdd(this, _e2, U({}));
    __privateAdd(this, _t2, U(null));
    __privateAdd(this, _n, U(null));
    __privateAdd(this, _a2, U({}));
    __privateAdd(this, _r, U({ id: null }));
    __privateAdd(this, _o, U({}));
    __privateAdd(this, _s, U(-1));
    __privateAdd(this, _i, U(new URL("https://example.com")));
  }
  get data() {
    return T(__privateGet(this, _e2));
  }
  set data(t) {
    I(__privateGet(this, _e2), t);
  }
  get form() {
    return T(__privateGet(this, _t2));
  }
  set form(t) {
    I(__privateGet(this, _t2), t);
  }
  get error() {
    return T(__privateGet(this, _n));
  }
  set error(t) {
    I(__privateGet(this, _n), t);
  }
  get params() {
    return T(__privateGet(this, _a2));
  }
  set params(t) {
    I(__privateGet(this, _a2), t);
  }
  get route() {
    return T(__privateGet(this, _r));
  }
  set route(t) {
    I(__privateGet(this, _r), t);
  }
  get state() {
    return T(__privateGet(this, _o));
  }
  set state(t) {
    I(__privateGet(this, _o), t);
  }
  get status() {
    return T(__privateGet(this, _s));
  }
  set status(t) {
    I(__privateGet(this, _s), t);
  }
  get url() {
    return T(__privateGet(this, _i));
  }
  set url(t) {
    I(__privateGet(this, _i), t);
  }
}, _e2 = new WeakMap(), _t2 = new WeakMap(), _n = new WeakMap(), _a2 = new WeakMap(), _r = new WeakMap(), _o = new WeakMap(), _s = new WeakMap(), _i = new WeakMap(), _c)(), Y = new (_d = class {
  constructor() {
    __privateAdd(this, _e3, U(null));
  }
  get current() {
    return T(__privateGet(this, _e3));
  }
  set current(t) {
    I(__privateGet(this, _e3), t);
  }
}, _e3 = new WeakMap(), _d)(), pe = new (_e5 = class {
  constructor() {
    __privateAdd(this, _e4, U(false));
  }
  get current() {
    return T(__privateGet(this, _e4));
  }
  set current(t) {
    I(__privateGet(this, _e4), t);
  }
}, _e4 = new WeakMap(), _e5)(), Ze.v = () => pe.current = true);
function tt(e) {
  Object.assign(k, e);
}
const Nt = /* @__PURE__ */ new Set(["icon", "shortcut icon", "apple-touch-icon"]), D = We(ze) ?? {}, z = We(Ye) ?? {}, C = { url: Be({}), page: Be({}), navigating: be(null), updated: Ot() };
function Le(e) {
  D[e] = ce();
}
function Dt(e, t) {
  let n = e + 1;
  for (; D[n]; ) delete D[n], n += 1;
  for (n = t + 1; z[n]; ) delete z[n], n += 1;
}
function H(e, t = false) {
  return t ? location.replace(e.href) : location.href = e.href, new Promise(() => {
  });
}
async function nt() {
  if ("serviceWorker" in navigator) {
    const e = await navigator.serviceWorker.getRegistration(A || "/");
    e && await e.update();
  }
}
function Ke() {
}
let Ue, me, te, O, we, v;
const ne = [], ae = [];
let R = null;
function ve() {
  var _a3;
  (_a3 = R == null ? void 0 : R.fork) == null ? void 0 : _a3.then((e) => e == null ? void 0 : e.discard()), R = null;
}
const Q = /* @__PURE__ */ new Map(), at = /* @__PURE__ */ new Set(), qt = /* @__PURE__ */ new Set(), G = /* @__PURE__ */ new Set();
let _ = { branch: [], error: null, url: null }, rt = false, re = false, Me = true, J = false, M = false, ot = false, Te = false, Ie, y, x, N;
const oe = /* @__PURE__ */ new Set(), Fe = /* @__PURE__ */ new Map();
async function ln(e, t, n) {
  var _a3, _b2, _c2, _d2, _e6;
  ((_a3 = globalThis.__sveltekit_uib792) == null ? void 0 : _a3.data) && globalThis.__sveltekit_uib792.data, document.URL !== location.href && (location.href = location.href), v = e, await ((_c2 = (_b2 = e.hooks).init) == null ? void 0 : _c2.call(_b2)), Ue = Lt(e), O = document.documentElement, we = t, me = e.nodes[0], te = e.nodes[1], me(), te(), y = (_d2 = history.state) == null ? void 0 : _d2[B], x = (_e6 = history.state) == null ? void 0 : _e6[W], y || (y = x = Date.now(), history.replaceState({ ...history.state, [B]: y, [W]: x }, ""));
  const a = D[y];
  function r() {
    a && (history.scrollRestoration = "manual", scrollTo(a.x, a.y));
  }
  n ? (r(), await Zt(we, n)) : (await K({ type: "enter", url: Je(v.hash ? nn(new URL(location.href)) : location.href), replace_state: true }), r()), Qt();
}
function Vt() {
  ne.length = 0, Te = false;
}
function st(e) {
  ae.some((t) => t == null ? void 0 : t.snapshot) && (z[e] = ae.map((t) => {
    var _a3;
    return (_a3 = t == null ? void 0 : t.snapshot) == null ? void 0 : _a3.capture();
  }));
}
function it(e) {
  var _a3;
  (_a3 = z[e]) == null ? void 0 : _a3.forEach((t, n) => {
    var _a4, _b2;
    (_b2 = (_a4 = ae[n]) == null ? void 0 : _a4.snapshot) == null ? void 0 : _b2.restore(t);
  });
}
function Ge() {
  Le(y), qe(ze, D), st(x), qe(Ye, z);
}
async function Bt(e, t, n, a) {
  let r;
  t.invalidateAll && ve(), await K({ type: "goto", url: Je(e), keepfocus: t.keepFocus, noscroll: t.noScroll, replace_state: t.replaceState, state: t.state, redirect_count: n, nav_token: a, accept: () => {
    t.invalidateAll && (Te = true, r = [...Fe.keys()]), t.invalidate && t.invalidate.forEach(Xt);
  } }), t.invalidateAll && Z().then(Z).then(() => {
    Fe.forEach(({ resource: s }, o) => {
      var _a3;
      (r == null ? void 0 : r.includes(o)) && ((_a3 = s.refresh) == null ? void 0 : _a3.call(s));
    });
  });
}
async function Kt(e) {
  if (e.id !== (R == null ? void 0 : R.id)) {
    ve();
    const t = {};
    if (oe.add(t), R = { id: e.id, token: t, promise: lt({ ...e, preload: t }).then((n) => (oe.delete(t), n.type === "loaded" && n.state.error && ve(), n)), fork: null }, De) {
      const n = R;
      n.fork = n.promise.then((a) => {
        if (n === R && a.type === "loaded") try {
          return De(() => {
            Ie.$set(a.props), tt(a.props.page);
          });
        } catch {
        }
        return null;
      });
    }
  }
  return R.promise;
}
async function ge(e) {
  var _a3;
  const t = (_a3 = await ue(e, false)) == null ? void 0 : _a3.route;
  t && await Promise.all([...t.layouts, t.leaf].map((n) => n == null ? void 0 : n[1]()));
}
async function ct(e, t, n) {
  var _a3;
  _ = e.state;
  const a = document.querySelector("style[data-sveltekit]");
  if (a && a.remove(), Object.assign(k, e.props.page), Ie = new v.root({ target: t, props: { ...e.props, stores: C, components: ae }, hydrate: n, sync: false }), await Promise.resolve(), it(x), n) {
    const r = { from: null, to: { params: _.params, route: { id: ((_a3 = _.route) == null ? void 0 : _a3.id) ?? null }, url: new URL(location.href) }, willUnload: false, type: "enter", complete: Promise.resolve() };
    G.forEach((s) => s(r));
  }
  re = true;
}
function se({ url: e, params: t, branch: n, status: a, error: r, route: s, form: o }) {
  let i = "never";
  if (A && (e.pathname === A || e.pathname === A + "/")) i = "always";
  else for (const u of n) (u == null ? void 0 : u.slash) !== void 0 && (i = u.slash);
  e.pathname = gt(e.pathname, i), e.search = e.search;
  const c = { type: "loaded", state: { url: e, params: t, branch: n, error: r, route: s }, props: { constructors: $t(n).map((u) => u.node.component), page: je(k) } };
  o !== void 0 && (c.props.form = o);
  let l = {}, f = !k, h = 0;
  for (let u = 0; u < Math.max(n.length, _.branch.length); u += 1) {
    const g = n[u], m = _.branch[u];
    (g == null ? void 0 : g.data) !== (m == null ? void 0 : m.data) && (f = true), g && (l = { ...l, ...g.data }, f && (c.props[`data_${h}`] = l), h += 1);
  }
  return (!_.url || e.href !== _.url.href || _.error !== r || o !== void 0 && o !== k.form || f) && (c.props.page = { error: r, params: t, route: { id: (s == null ? void 0 : s.id) ?? null }, state: {}, status: a, url: new URL(e), form: o ?? null, data: f ? l : k.data }), c;
}
async function Oe({ loader: e, parent: t, url: n, params: a, route: r, server_data_node: s }) {
  var _a3, _b2;
  let o = null;
  const i = { dependencies: /* @__PURE__ */ new Set(), params: /* @__PURE__ */ new Set(), parent: false, route: false, url: false, search_params: /* @__PURE__ */ new Set() }, c = await e();
  return { node: c, loader: e, server: s, universal: ((_a3 = c.universal) == null ? void 0 : _a3.load) ? { type: "data", data: o, uses: i } : null, data: o ?? (s == null ? void 0 : s.data) ?? null, slash: ((_b2 = c.universal) == null ? void 0 : _b2.trailingSlash) ?? (s == null ? void 0 : s.slash) };
}
function Mt(e, t, n) {
  let a = e instanceof Request ? e.url : e;
  const r = new URL(a, n);
  r.origin === n.origin && (a = r.href.slice(n.origin.length));
  const s = re ? kt(a, r.href, t) : bt(a, t);
  return { resolved: r, promise: s };
}
function Ft(e, t, n, a, r, s) {
  if (Te) return true;
  if (!r) return false;
  if (r.parent && e || r.route && t || r.url && n) return true;
  for (const o of r.search_params) if (a.has(o)) return true;
  for (const o of r.params) if (s[o] !== _.params[o]) return true;
  for (const o of r.dependencies) if (ne.some((i) => i(new URL(o)))) return true;
  return false;
}
function Pe(e, t) {
  return (e == null ? void 0 : e.type) === "data" ? e : (e == null ? void 0 : e.type) === "skip" ? t ?? null : null;
}
function Gt(e, t) {
  if (!e) return new Set(t.searchParams.keys());
  const n = /* @__PURE__ */ new Set([...e.searchParams.keys(), ...t.searchParams.keys()]);
  for (const a of n) {
    const r = e.searchParams.getAll(a), s = t.searchParams.getAll(a);
    r.every((o) => s.includes(o)) && s.every((o) => r.includes(o)) && n.delete(a);
  }
  return n;
}
function Wt({ error: e, url: t, route: n, params: a }) {
  return { type: "loaded", state: { error: e, url: t, route: n, params: a, branch: [] }, props: { page: je(k), constructors: [] } };
}
async function lt({ id: e, invalidating: t, url: n, params: a, route: r, preload: s }) {
  if ((R == null ? void 0 : R.id) === e) return oe.delete(R.token), R.promise;
  const { errors: o, layouts: i, leaf: c } = r, l = [...i, c];
  o.forEach((p) => p == null ? void 0 : p().catch(() => {
  })), l.forEach((p) => p == null ? void 0 : p[1]().catch(() => {
  }));
  const f = _.url ? e !== ie(_.url) : false, h = _.route ? r.id !== _.route.id : false, w = Gt(_.url, n);
  let u = false;
  const g = l.map(async (p, d) => {
    var _a3;
    if (!p) return;
    const S = _.branch[d];
    return p[1] === (S == null ? void 0 : S.loader) && !Ft(u, h, f, w, (_a3 = S.universal) == null ? void 0 : _a3.uses, a) ? S : (u = true, Oe({ loader: p[1], url: n, params: a, route: r, parent: async () => {
      var _a4;
      const P = {};
      for (let L = 0; L < d; L += 1) Object.assign(P, (_a4 = await g[L]) == null ? void 0 : _a4.data);
      return P;
    }, server_data_node: Pe(p[0] ? { type: "skip" } : null, p[0] ? S == null ? void 0 : S.server : void 0) }));
  });
  for (const p of g) p.catch(() => {
  });
  const m = [];
  for (let p = 0; p < l.length; p += 1) if (l[p]) try {
    m.push(await g[p]);
  } catch (d) {
    if (d instanceof Se) return { type: "redirect", location: d.location };
    if (oe.has(s)) return Wt({ error: await X(d, { params: a, url: n, route: { id: r.id } }), url: n, params: a, route: r });
    let S = Ae(d), E;
    if (d instanceof ke) E = d.body;
    else {
      if (await C.updated.check()) return await nt(), await H(n);
      E = await X(d, { params: a, url: n, route: { id: r.id } });
    }
    const P = await Yt(p, m, o);
    return P ? se({ url: n, params: a, branch: m.slice(0, P.idx).concat(P.node), status: S, error: E, route: r }) : await ft(n, { id: r.id }, E, S);
  }
  else m.push(void 0);
  return se({ url: n, params: a, branch: m, status: 200, error: null, route: r, form: t ? void 0 : null });
}
async function Yt(e, t, n) {
  for (; e--; ) if (n[e]) {
    let a = e;
    for (; !t[a]; ) a -= 1;
    try {
      return { idx: a + 1, node: { node: await n[e](), loader: n[e], data: {}, server: null, universal: null } };
    } catch {
      continue;
    }
  }
}
async function $e({ status: e, error: t, url: n, route: a }) {
  const r = {};
  let s = null;
  try {
    const o = await Oe({ loader: me, url: n, params: r, route: a, parent: () => Promise.resolve({}), server_data_node: Pe(s) }), i = { node: await te(), loader: te, universal: null, server: null, data: null };
    return se({ url: n, params: r, branch: [o, i], status: e, error: t, route: null });
  } catch (o) {
    if (o instanceof Se) return Bt(new URL(o.location, location.href), {}, 0);
    throw o;
  }
}
async function zt(e) {
  const t = e.href;
  if (Q.has(t)) return Q.get(t);
  let n;
  try {
    const a = (async () => {
      let r = await v.hooks.reroute({ url: new URL(e), fetch: async (s, o) => Mt(s, o, e).promise }) ?? e;
      if (typeof r == "string") {
        const s = new URL(e);
        v.hash ? s.hash = r : s.pathname = r, r = s;
      }
      return r;
    })();
    Q.set(t, a), n = await a;
  } catch {
    Q.delete(t);
    return;
  }
  return n;
}
async function ue(e, t) {
  if (e && !le(e, A, v.hash)) {
    const n = await zt(e);
    if (!n) return;
    const a = Ht(n);
    for (const r of Ue) {
      const s = r.exec(a);
      if (s) return { id: ie(e), invalidating: t, route: r, params: mt(s), url: e };
    }
  }
}
function Ht(e) {
  return _t(v.hash ? e.hash.replace(/^#/, "").replace(/[?#].+/, "") : e.pathname.slice(A.length)) || "/";
}
function ie(e) {
  return (v.hash ? e.hash.replace(/^#/, "") : e.pathname) + e.search;
}
function ut({ url: e, type: t, intent: n, delta: a, event: r }) {
  let s = false;
  const o = Ce(_, n, e, t);
  a !== void 0 && (o.navigation.delta = a), r !== void 0 && (o.navigation.event = r);
  const i = { ...o.navigation, cancel: () => {
    s = true, o.reject(new Error("navigation cancelled"));
  } };
  return J || at.forEach((c) => c(i)), s ? null : o;
}
async function K({ type: e, url: t, popped: n, keepfocus: a, noscroll: r, replace_state: s, state: o = {}, redirect_count: i = 0, nav_token: c = {}, accept: l = Ke, block: f = Ke, event: h }) {
  var _a3;
  const w = N;
  N = c;
  const u = await ue(t, false), g = e === "enter" ? Ce(_, u, t, e) : ut({ url: t, type: e, delta: n == null ? void 0 : n.delta, intent: u, event: h });
  if (!g) {
    f(), N === c && (N = w);
    return;
  }
  const m = y, p = x;
  l(), J = true, re && g.navigation.type !== "enter" && C.navigating.set(Y.current = g.navigation);
  let d = u && await lt(u);
  if (!d) {
    if (le(t, A, v.hash)) return await H(t, s);
    d = await ft(t, { id: null }, await X(new Ee(404, "Not Found", `Not found: ${t.pathname}`), { url: t, params: {}, route: { id: null } }), 404, s);
  }
  if (t = (u == null ? void 0 : u.url) || t, N !== c) return g.reject(new Error("navigation aborted")), false;
  if (d.type === "redirect") {
    if (i < 20) {
      await K({ type: e, url: new URL(d.location, t), popped: n, keepfocus: a, noscroll: r, replace_state: s, state: o, redirect_count: i + 1, nav_token: c }), g.fulfil(void 0);
      return;
    }
    d = await $e({ status: 500, error: await X(new Error("Redirect loop"), { url: t, params: {}, route: { id: null } }), url: t, route: { id: null } });
  } else d.props.page.status >= 400 && await C.updated.check() && (await nt(), await H(t, s));
  if (Vt(), Le(m), st(p), d.props.page.url.pathname !== t.pathname && (t.pathname = d.props.page.url.pathname), o = n ? n.state : o, !n) {
    const b = s ? 0 : 1, q = { [B]: y += b, [W]: x += b, [He]: o };
    (s ? history.replaceState : history.pushState).call(history, q, "", t), s || Dt(y, x);
  }
  const S = u && (R == null ? void 0 : R.id) === u.id ? R.fork : null;
  R = null, d.props.page.state = o;
  let E;
  if (re) {
    const b = (await Promise.all(Array.from(qt, ($) => $(g.navigation)))).filter(($) => typeof $ == "function");
    if (b.length > 0) {
      let $ = function() {
        b.forEach((fe) => {
          G.delete(fe);
        });
      };
      b.push($), b.forEach((fe) => {
        G.add(fe);
      });
    }
    _ = d.state, d.props.page && (d.props.page.url = t);
    const q = S && await S;
    q ? E = q.commit() : (Ie.$set(d.props), tt(d.props.page), E = (_a3 = pt) == null ? void 0 : _a3()), ot = true;
  } else await ct(d, we, false);
  const { activeElement: P } = document;
  await E, await Z(), await Z();
  let L = n ? n.scroll : r ? ce() : null;
  if (Me) {
    const b = t.hash && document.getElementById(dt(t));
    if (L) scrollTo(L.x, L.y);
    else if (b) {
      b.scrollIntoView();
      const { top: q, left: $ } = b.getBoundingClientRect();
      L = { x: pageXOffset + $, y: pageYOffset + q };
    } else scrollTo(0, 0);
  }
  const ht = document.activeElement !== P && document.activeElement !== document.body;
  !a && !ht && tn(t, L), Me = true, d.props.page && Object.assign(k, d.props.page), J = false, e === "popstate" && it(x), g.fulfil(void 0), G.forEach((b) => b(g.navigation)), C.navigating.set(Y.current = null);
}
async function ft(e, t, n, a, r) {
  return e.origin === xe && e.pathname === location.pathname && !rt ? await $e({ status: a, error: n, url: e, route: t }) : await H(e, r);
}
function Jt() {
  let e, t, n;
  O.addEventListener("mousemove", (i) => {
    const c = i.target;
    clearTimeout(e), e = setTimeout(() => {
      s(c, j.hover);
    }, 20);
  });
  function a(i) {
    i.defaultPrevented || s(i.composedPath()[0], j.tap);
  }
  O.addEventListener("mousedown", a), O.addEventListener("touchstart", a, { passive: true });
  const r = new IntersectionObserver((i) => {
    for (const c of i) c.isIntersecting && (ge(new URL(c.target.href)), r.unobserve(c.target));
  }, { threshold: 0 });
  async function s(i, c) {
    const l = Qe(i, O), f = l === t && c >= n;
    if (!l || f) return;
    const { url: h, external: w, download: u } = _e(l, A, v.hash);
    if (w || u) return;
    const g = ee(l), m = h && ie(_.url) === ie(h);
    if (!(g.reload || m)) if (c <= g.preload_data) {
      t = l, n = j.tap;
      const p = await ue(h, false);
      if (!p) return;
      Kt(p);
    } else c <= g.preload_code && (t = l, n = c, ge(h));
  }
  function o() {
    r.disconnect();
    for (const i of O.querySelectorAll("a")) {
      const { url: c, external: l, download: f } = _e(i, A, v.hash);
      if (l || f) continue;
      const h = ee(i);
      h.reload || (h.preload_code === j.viewport && r.observe(i), h.preload_code === j.eager && ge(c));
    }
  }
  G.add(o), o();
}
function X(e, t) {
  if (e instanceof ke) return e.body;
  const n = Ae(e), a = Ct(e);
  return v.hooks.handleError({ error: e, event: t, status: n, message: a }) ?? { message: a };
}
function Xt(e) {
  if (typeof e == "function") ne.push(e);
  else {
    const { href: t } = new URL(e, location.href);
    ne.push((n) => n.href === t);
  }
}
function Qt() {
  var _a3;
  history.scrollRestoration = "manual", addEventListener("beforeunload", (t) => {
    let n = false;
    if (Ge(), !J) {
      const a = Ce(_, void 0, null, "leave"), r = { ...a.navigation, cancel: () => {
        n = true, a.reject(new Error("navigation cancelled"));
      } };
      at.forEach((s) => s(r));
    }
    n ? (t.preventDefault(), t.returnValue = "") : history.scrollRestoration = "auto";
  }), addEventListener("visibilitychange", () => {
    document.visibilityState === "hidden" && Ge();
  }), ((_a3 = navigator.connection) == null ? void 0 : _a3.saveData) || Jt(), O.addEventListener("click", async (t) => {
    if (t.button || t.which !== 1 || t.metaKey || t.ctrlKey || t.shiftKey || t.altKey || t.defaultPrevented) return;
    const n = Qe(t.composedPath()[0], O);
    if (!n) return;
    const { url: a, external: r, target: s, download: o } = _e(n, A, v.hash);
    if (!a) return;
    if (s === "_parent" || s === "_top") {
      if (window.parent !== window) return;
    } else if (s && s !== "_self") return;
    const i = ee(n);
    if (!(n instanceof SVGAElement) && a.protocol !== location.protocol && !(a.protocol === "https:" || a.protocol === "http:") || o) return;
    const [l, f] = (v.hash ? a.hash.replace(/^#/, "") : a.href).split("#"), h = l === de(location);
    if (r || i.reload && (!h || !f)) {
      ut({ url: a, type: "link", event: t }) ? J = true : t.preventDefault();
      return;
    }
    if (f !== void 0 && h) {
      const [, w] = _.url.href.split("#");
      if (w === f) {
        if (t.preventDefault(), f === "" || f === "top" && n.ownerDocument.getElementById("top") === null) scrollTo({ top: 0 });
        else {
          const u = n.ownerDocument.getElementById(decodeURIComponent(f));
          u && (u.scrollIntoView(), u.focus());
        }
        return;
      }
      if (M = true, Le(y), e(a), !i.replace_state) return;
      M = false;
    }
    t.preventDefault(), await new Promise((w) => {
      requestAnimationFrame(() => {
        setTimeout(w, 0);
      }), setTimeout(w, 100);
    }), await K({ type: "link", url: a, keepfocus: i.keepfocus, noscroll: i.noscroll, replace_state: i.replace_state ?? a.href === location.href, event: t });
  }), O.addEventListener("submit", (t) => {
    if (t.defaultPrevented) return;
    const n = HTMLFormElement.prototype.cloneNode.call(t.target), a = t.submitter;
    if (((a == null ? void 0 : a.formTarget) || n.target) === "_blank" || ((a == null ? void 0 : a.formMethod) || n.method) !== "get") return;
    const o = new URL((a == null ? void 0 : a.hasAttribute("formaction")) && (a == null ? void 0 : a.formAction) || n.action);
    if (le(o, A, false)) return;
    const i = t.target, c = ee(i);
    if (c.reload) return;
    t.preventDefault(), t.stopPropagation();
    const l = new FormData(i, a);
    o.search = new URLSearchParams(l).toString(), K({ type: "form", url: o, keepfocus: c.keepfocus, noscroll: c.noscroll, replace_state: c.replace_state ?? o.href === location.href, event: t });
  }), addEventListener("popstate", async (t) => {
    var _a4;
    if (!ye) {
      if ((_a4 = t.state) == null ? void 0 : _a4[B]) {
        const n = t.state[B];
        if (N = {}, n === y) return;
        const a = D[n], r = t.state[He] ?? {}, s = new URL(t.state[It] ?? location.href), o = t.state[W], i = _.url ? de(location) === de(_.url) : false;
        if (o === x && (ot || i)) {
          r !== k.state && (k.state = r), e(s), D[y] = ce(), a && scrollTo(a.x, a.y), y = n;
          return;
        }
        const l = n - y;
        await K({ type: "popstate", url: s, popped: { state: r, scroll: a, delta: l }, accept: () => {
          y = n, x = o;
        }, block: () => {
          history.go(-l);
        }, nav_token: N, event: t });
      } else if (!M) {
        const n = new URL(location.href);
        e(n), v.hash && location.reload();
      }
    }
  }), addEventListener("hashchange", () => {
    M && (M = false, history.replaceState({ ...history.state, [B]: ++y, [W]: x }, "", location.href));
  });
  for (const t of document.querySelectorAll("link")) Nt.has(t.rel) && (t.href = t.href);
  addEventListener("pageshow", (t) => {
    t.persisted && C.navigating.set(Y.current = null);
  });
  function e(t) {
    _.url = k.url = t, C.page.set(je(k)), C.page.notify();
  }
}
async function Zt(e, { status: t = 200, error: n, node_ids: a, params: r, route: s, server_route: o, data: i, form: c }) {
  rt = true;
  const l = new URL(location.href);
  let f;
  ({ params: r = {}, route: s = { id: null } } = await ue(l, false) || {}), f = Ue.find(({ id: u }) => u === s.id);
  let h, w = true;
  try {
    const u = a.map(async (m, p) => {
      const d = i[p];
      return (d == null ? void 0 : d.uses) && (d.uses = en(d.uses)), Oe({ loader: v.nodes[m], url: l, params: r, route: s, parent: async () => {
        const S = {};
        for (let E = 0; E < p; E += 1) Object.assign(S, (await u[E]).data);
        return S;
      }, server_data_node: Pe(d) });
    }), g = await Promise.all(u);
    if (f) {
      const m = f.layouts;
      for (let p = 0; p < m.length; p++) m[p] || g.splice(p, 0, void 0);
    }
    h = se({ url: l, params: r, branch: g, status: t, error: n, form: c, route: f ?? null });
  } catch (u) {
    if (u instanceof Se) {
      await H(new URL(u.location, location.href));
      return;
    }
    h = await $e({ status: Ae(u), error: await X(u, { url: l, params: r, route: s }), url: l, route: s }), e.textContent = "", w = false;
  }
  h.props.page && (h.props.page.state = {}), await ct(h, e, w);
}
function en(e) {
  return { dependencies: new Set((e == null ? void 0 : e.dependencies) ?? []), params: new Set((e == null ? void 0 : e.params) ?? []), parent: !!(e == null ? void 0 : e.parent), route: !!(e == null ? void 0 : e.route), url: !!(e == null ? void 0 : e.url), search_params: new Set((e == null ? void 0 : e.search_params) ?? []) };
}
let ye = false;
function tn(e, t = null) {
  const n = document.querySelector("[autofocus]");
  if (n) n.focus();
  else {
    const a = dt(e);
    if (a && document.getElementById(a)) {
      const { x: s, y: o } = t ?? ce();
      setTimeout(() => {
        const i = history.state;
        ye = true, location.replace(`#${a}`), v.hash && location.replace(e.hash), history.replaceState(i, "", e.hash), scrollTo(s, o), ye = false;
      });
    } else {
      const s = document.body, o = s.getAttribute("tabindex");
      s.tabIndex = -1, s.focus({ preventScroll: true, focusVisible: false }), o !== null ? s.setAttribute("tabindex", o) : s.removeAttribute("tabindex");
    }
    const r = getSelection();
    if (r && r.type !== "None") {
      const s = [];
      for (let o = 0; o < r.rangeCount; o += 1) s.push(r.getRangeAt(o));
      setTimeout(() => {
        if (r.rangeCount === s.length) {
          for (let o = 0; o < r.rangeCount; o += 1) {
            const i = s[o], c = r.getRangeAt(o);
            if (i.commonAncestorContainer !== c.commonAncestorContainer || i.startContainer !== c.startContainer || i.endContainer !== c.endContainer || i.startOffset !== c.startOffset || i.endOffset !== c.endOffset) return;
          }
          r.removeAllRanges();
        }
      });
    }
  }
}
function Ce(e, t, n, a) {
  var _a3, _b2;
  let r, s;
  const o = new Promise((c, l) => {
    r = c, s = l;
  });
  return o.catch(() => {
  }), { navigation: { from: { params: e.params, route: { id: ((_a3 = e.route) == null ? void 0 : _a3.id) ?? null }, url: e.url }, to: n && { params: (t == null ? void 0 : t.params) ?? null, route: { id: ((_b2 = t == null ? void 0 : t.route) == null ? void 0 : _b2.id) ?? null }, url: n }, willUnload: !t, type: a, complete: o }, fulfil: r, reject: s };
}
function je(e) {
  return { data: e.data, error: e.error, form: e.form, params: e.params, route: e.route, state: e.state, status: e.status, url: e.url };
}
function nn(e) {
  const t = new URL(e);
  return t.hash = decodeURIComponent(e.hash), t;
}
function dt(e) {
  let t;
  if (v.hash) {
    const [, , n] = e.hash.split("#", 3);
    t = n ?? "";
  } else t = e.hash.slice(1);
  return decodeURIComponent(t);
}
export {
  ln as a,
  rn as l,
  k as p,
  C as s
};
