import { m as Ee, n as R, o as Je, g as L, R as Ze, q as $e, v as ce, w as $, x as W, B as z, ak as et, y as Ae, z as le, A as tt, aq as ee, ao as me, a9 as K, ar as ve, I as rt, as as de, aa as at, at as fe, au as it, an as nt, av as st, aw as Ce, ax as ue, ay as ot, ad as lt, ae as ft, ab as ut, az as Te, F as Re, aA as ct, E as Le, G as x, aB as vt, aC as dt, aD as ht, aE as _t, aF as gt, aG as bt, aH as wt, C as yt, b as G, aI as pt, aJ as kt, aK as Et, aL as At, a6 as he, a7 as _e, a8 as mt, D as Se, aM as Ct, k as N, s as j, l as I, t as J, p as Ne, a3 as Y, a as Ie, j as Me, N as S, a5 as te, aN as Tt, W as se } from "./DEAb5m-A.js";
import { i as Rt, d as Lt, e as Oe, f as St, n as Nt, g as It, j as Mt, k as Ot, w as Bt, l as Be, t as U, a as F, o as qt } from "./B3zjZD7z.js";
import { p as T, i as X, b as qe, r as Ft } from "./COt1BKSo.js";
import "./ByraeRS2.js";
function _r(e, t) {
  return t;
}
function Dt(e, t, r, a) {
  for (var i = [], n = t.length, s = 0; s < n; s++) it(t[s].e, i, true);
  var o = n > 0 && i.length === 0 && r !== null;
  if (o) {
    var w = r.parentNode;
    nt(w), w.append(r), a.clear(), B(e, t[0].prev, t[n - 1].next);
  }
  st(i, () => {
    for (var l = 0; l < n; l++) {
      var c = t[l];
      o || (a.delete(c.k), B(e, c.prev, c.next)), Ce(c.e, !o);
    }
  });
}
function gr(e, t, r, a, i, n = null) {
  var s = e, o = { flags: t, items: /* @__PURE__ */ new Map(), first: null }, w = (t & Te) !== 0;
  if (w) {
    var l = e;
    s = R ? $(ft(l)) : l.appendChild(ut());
  }
  R && Je();
  var c = null, p = false, f = Ze(() => {
    var g = r();
    return at(g) ? g : g == null ? [] : me(g);
  });
  Ee(() => {
    var g = L(f), u = g.length;
    if (p && u === 0) return;
    p = u === 0;
    let y = false;
    if (R) {
      var E = s.data === $e;
      E !== (u === 0) && (s = ce(), $(s), W(false), y = true);
    }
    if (R) {
      for (var d = null, A, h = 0; h < u; h++) {
        if (z.nodeType === 8 && z.data === et) {
          s = z, y = true, W(false);
          break;
        }
        var v = g[h], b = a(v, h);
        A = Fe(z, o, d, null, v, b, h, i, t, r), o.items.set(b, A), d = A;
      }
      u > 0 && $(ce());
    }
    R || Ut(g, o, s, i, t, a, r), n !== null && (u === 0 ? c ? Ae(c) : c = le(() => n(s)) : c !== null && tt(c, () => {
      c = null;
    })), y && W(true), L(f);
  }), R && (s = z);
}
function Ut(e, t, r, a, i, n, s) {
  var _a, _b, _c, _d;
  var o = (i & ct) !== 0, w = (i & (ue | fe)) !== 0, l = e.length, c = t.items, p = t.first, f = p, g, u = null, y, E = [], d = [], A, h, v, b;
  if (o) for (b = 0; b < l; b += 1) A = e[b], h = n(A, b), v = c.get(h), v !== void 0 && ((_a = v.a) == null ? void 0 : _a.measure(), (y ?? (y = /* @__PURE__ */ new Set())).add(v));
  for (b = 0; b < l; b += 1) {
    if (A = e[b], h = n(A, b), v = c.get(h), v === void 0) {
      var k = f ? f.e.nodes_start : r;
      u = Fe(k, t, u, u === null ? t.first : u.next, A, h, b, a, i, s), c.set(h, u), E = [], d = [], f = u.next;
      continue;
    }
    if (w && zt(v, A, b, i), (v.e.f & ee) !== 0 && (Ae(v.e), o && ((_b = v.a) == null ? void 0 : _b.unfix(), (y ?? (y = /* @__PURE__ */ new Set())).delete(v))), v !== f) {
      if (g !== void 0 && g.has(v)) {
        if (E.length < d.length) {
          var _ = d[0], m;
          u = _.prev;
          var C = E[0], O = E[E.length - 1];
          for (m = 0; m < E.length; m += 1) ge(E[m], _, r);
          for (m = 0; m < d.length; m += 1) g.delete(d[m]);
          B(t, C.prev, O.next), B(t, u, C), B(t, O, _), f = _, u = O, b -= 1, E = [], d = [];
        } else g.delete(v), ge(v, f, r), B(t, v.prev, v.next), B(t, v, u === null ? t.first : u.next), B(t, u, v), u = v;
        continue;
      }
      for (E = [], d = []; f !== null && f.k !== h; ) (f.e.f & ee) === 0 && (g ?? (g = /* @__PURE__ */ new Set())).add(f), d.push(f), f = f.next;
      if (f === null) continue;
      v = f;
    }
    E.push(v), u = v, f = v.next;
  }
  if (f !== null || g !== void 0) {
    for (var D = g === void 0 ? [] : me(g); f !== null; ) (f.e.f & ee) === 0 && D.push(f), f = f.next;
    var Z = D.length;
    if (Z > 0) {
      var Xe = (i & Te) !== 0 && l === 0 ? r : null;
      if (o) {
        for (b = 0; b < Z; b += 1) (_c = D[b].a) == null ? void 0 : _c.measure();
        for (b = 0; b < Z; b += 1) (_d = D[b].a) == null ? void 0 : _d.fix();
      }
      Dt(t, D, Xe, c);
    }
  }
  o && Re(() => {
    var _a2;
    if (y !== void 0) for (v of y) (_a2 = v.a) == null ? void 0 : _a2.apply();
  }), K.first = t.first && t.first.e, K.last = u && u.e;
}
function zt(e, t, r, a) {
  (a & ue) !== 0 && ve(e.v, t), (a & fe) !== 0 ? ve(e.i, r) : e.i = r;
}
function Fe(e, t, r, a, i, n, s, o, w, l) {
  var c = (w & ue) !== 0, p = (w & ot) === 0, f = c ? p ? rt(i) : de(i) : i, g = (w & fe) === 0 ? s : de(s), u = { i: g, v: f, k: n, a: null, e: null, prev: r, next: a };
  try {
    return u.e = le(() => o(e, f, g, l), R), u.e.prev = r && r.e, u.e.next = a && a.e, r === null ? t.first = u : (r.next = u, r.e.next = u.e), a !== null && (a.prev = u, a.e.prev = u.e), u;
  } finally {
  }
}
function ge(e, t, r) {
  for (var a = e.next ? e.next.e.nodes_start : r, i = t ? t.e.nodes_start : r, n = e.e.nodes_start; n !== a; ) {
    var s = lt(n);
    i.before(n), n = s;
  }
}
function B(e, t, r) {
  t === null ? e.first = r : (t.next = r, t.e.next = r && r.e), r !== null && (r.prev = t, r.e.prev = t && t.e);
}
function De(e, t, ...r) {
  var a = e, i = x, n;
  Ee(() => {
    i !== (i = t()) && (n && (Ce(n), n = null), n = le(() => i(a, ...r)));
  }, Le), R && (a = z);
}
function Ue(e) {
  var t, r, a = "";
  if (typeof e == "string" || typeof e == "number") a += e;
  else if (typeof e == "object") if (Array.isArray(e)) {
    var i = e.length;
    for (t = 0; t < i; t++) e[t] && (r = Ue(e[t])) && (a && (a += " "), a += r);
  } else for (r in e) e[r] && (a && (a += " "), a += r);
  return a;
}
function xt() {
  for (var e, t, r = 0, a = "", i = arguments.length; r < i; r++) (e = arguments[r]) && (t = Ue(e)) && (a && (a += " "), a += t);
  return a;
}
function Pt(e) {
  return typeof e == "object" ? xt(e) : e ?? "";
}
const be = [...` 	
\r\f\xA0\v\uFEFF`];
function Ht(e, t, r) {
  var a = e == null ? "" : "" + e;
  if (t && (a = a ? a + " " + t : t), r) {
    for (var i in r) if (r[i]) a = a ? a + " " + i : i;
    else if (a.length) for (var n = i.length, s = 0; (s = a.indexOf(i, s)) >= 0; ) {
      var o = s + n;
      (s === 0 || be.includes(a[s - 1])) && (o === a.length || be.includes(a[o])) ? a = (s === 0 ? "" : a.substring(0, s)) + a.substring(o + 1) : s = o;
    }
  }
  return a === "" ? null : a;
}
function we(e, t = false) {
  var r = t ? " !important;" : ";", a = "";
  for (var i in e) {
    var n = e[i];
    n != null && n !== "" && (a += " " + i + ": " + n + r);
  }
  return a;
}
function re(e) {
  return e[0] !== "-" || e[1] !== "-" ? e.toLowerCase() : e;
}
function Yt(e, t) {
  if (t) {
    var r = "", a, i;
    if (Array.isArray(t) ? (a = t[0], i = t[1]) : a = t, e) {
      e = String(e).replaceAll(/\s*\/\*.*?\*\/\s*/g, "").trim();
      var n = false, s = 0, o = false, w = [];
      a && w.push(...Object.keys(a).map(re)), i && w.push(...Object.keys(i).map(re));
      var l = 0, c = -1;
      const y = e.length;
      for (var p = 0; p < y; p++) {
        var f = e[p];
        if (o ? f === "/" && e[p - 1] === "*" && (o = false) : n ? n === f && (n = false) : f === "/" && e[p + 1] === "*" ? o = true : f === '"' || f === "'" ? n = f : f === "(" ? s++ : f === ")" && s--, !o && n === false && s === 0) {
          if (f === ":" && c === -1) c = p;
          else if (f === ";" || p === y - 1) {
            if (c !== -1) {
              var g = re(e.substring(l, c).trim());
              if (!w.includes(g)) {
                f !== ";" && p++;
                var u = e.substring(l, p).trim();
                r += " " + u + ";";
              }
            }
            l = p + 1, c = -1;
          }
        }
      }
    }
    return a && (r += we(a)), i && (r += we(i, true)), r = r.trim(), r === "" ? null : r;
  }
  return e == null ? null : String(e);
}
function ze(e, t, r, a, i, n) {
  var s = e.__className;
  if (R || s !== r || s === void 0) {
    var o = Ht(r, a, n);
    (!R || o !== e.getAttribute("class")) && (o == null ? e.removeAttribute("class") : t ? e.className = o : e.setAttribute("class", o)), e.__className = r;
  } else if (n && i !== n) for (var w in n) {
    var l = !!n[w];
    (i == null || l !== !!i[w]) && e.classList.toggle(w, l);
  }
  return n;
}
function ae(e, t = {}, r, a) {
  for (var i in r) {
    var n = r[i];
    t[i] !== n && (r[i] == null ? e.style.removeProperty(i) : e.style.setProperty(i, n, a));
  }
}
function H(e, t, r, a) {
  var i = e.__style;
  if (R || i !== t) {
    var n = Yt(t, a);
    (!R || n !== e.getAttribute("style")) && (n == null ? e.removeAttribute("style") : e.style.cssText = n), e.__style = t;
  } else a && (Array.isArray(a) ? (ae(e, r == null ? void 0 : r[0], a[0]), ae(e, r == null ? void 0 : r[1], a[1], "important")) : ae(e, r, a));
  return a;
}
const V = Symbol("class"), P = Symbol("style"), xe = Symbol("is custom element"), Pe = Symbol("is html");
function br(e) {
  if (R) {
    var t = false, r = () => {
      if (!t) {
        if (t = true, e.hasAttribute("value")) {
          var a = e.value;
          q(e, "value", null), e.value = a;
        }
        if (e.hasAttribute("checked")) {
          var i = e.checked;
          q(e, "checked", null), e.checked = i;
        }
      }
    };
    e.__on_r = r, gt(r), Mt();
  }
}
function Vt(e, t) {
  t ? e.hasAttribute("selected") || e.setAttribute("selected", "") : e.removeAttribute("selected");
}
function q(e, t, r, a) {
  var i = He(e);
  R && (i[t] = e.getAttribute(t), t === "src" || t === "srcset" || t === "href" && e.nodeName === "LINK") || i[t] !== (i[t] = r) && (t === "loading" && (e[_t] = r), r == null ? e.removeAttribute(t) : typeof r != "string" && Ye(e).includes(t) ? e[t] = r : e.setAttribute(t, r));
}
function Wt(e, t, r, a, i = false) {
  var n = He(e), s = n[xe], o = !n[Pe];
  let w = R && s;
  w && W(false);
  var l = t || {}, c = e.tagName === "OPTION";
  for (var p in t) p in r || (r[p] = null);
  r.class ? r.class = Pt(r.class) : r.class = null, r[P] && (r.style ?? (r.style = null));
  var f = Ye(e);
  for (const h in r) {
    let v = r[h];
    if (c && h === "value" && v == null) {
      e.value = e.__value = "", l[h] = v;
      continue;
    }
    if (h === "class") {
      var g = e.namespaceURI === "http://www.w3.org/1999/xhtml";
      ze(e, g, v, a, t == null ? void 0 : t[V], r[V]), l[h] = v, l[V] = r[V];
      continue;
    }
    if (h === "style") {
      H(e, v, t == null ? void 0 : t[P], r[P]), l[h] = v, l[P] = r[P];
      continue;
    }
    var u = l[h];
    if (v !== u) {
      l[h] = v;
      var y = h[0] + h[1];
      if (y !== "$$") if (y === "on") {
        const b = {}, k = "$$" + h;
        let _ = h.slice(2);
        var E = It(_);
        if (Rt(_) && (_ = _.slice(0, -7), b.capture = true), !E && u) {
          if (v != null) continue;
          e.removeEventListener(_, l[k], b), l[k] = null;
        }
        if (v != null) if (E) e[`__${_}`] = v, Oe([_]);
        else {
          let m = function(C) {
            l[h].call(this, C);
          };
          l[k] = Lt(_, e, m, b);
        }
        else E && (e[`__${_}`] = void 0);
      } else if (h === "style") q(e, h, v);
      else if (h === "autofocus") St(e, !!v);
      else if (!s && (h === "__value" || h === "value" && v != null)) e.value = e.__value = v;
      else if (h === "selected" && c) Vt(e, v);
      else {
        var d = h;
        o || (d = Nt(d));
        var A = d === "defaultValue" || d === "defaultChecked";
        if (v == null && !s && !A) if (n[h] = null, d === "value" || d === "checked") {
          let b = e;
          const k = t === void 0;
          if (d === "value") {
            let _ = b.defaultValue;
            b.removeAttribute(d), b.defaultValue = _, b.value = b.__value = k ? _ : null;
          } else {
            let _ = b.defaultChecked;
            b.removeAttribute(d), b.defaultChecked = _, b.checked = k ? _ : false;
          }
        } else e.removeAttribute(h);
        else A || f.includes(d) && (s || typeof v != "string") ? e[d] = v : typeof v != "function" && q(e, d, v);
      }
    }
  }
  return w && W(true), l;
}
function He(e) {
  return e.__attributes ?? (e.__attributes = { [xe]: e.nodeName.includes("-"), [Pe]: e.namespaceURI === vt });
}
var ye = /* @__PURE__ */ new Map();
function Ye(e) {
  var t = ye.get(e.nodeName);
  if (t) return t;
  ye.set(e.nodeName, t = []);
  for (var r, a = e, i = Element.prototype; i !== a; ) {
    r = ht(a);
    for (var n in r) r[n].set && t.push(n);
    a = dt(a);
  }
  return t;
}
const Gt = () => performance.now(), M = { tick: (e) => requestAnimationFrame(e), now: () => Gt(), tasks: /* @__PURE__ */ new Set() };
function Ve() {
  const e = M.now();
  M.tasks.forEach((t) => {
    t.c(e) || (M.tasks.delete(t), t.f());
  }), M.tasks.size !== 0 && M.tick(Ve);
}
function Qt(e) {
  let t;
  return M.tasks.size === 0 && M.tick(Ve), { promise: new Promise((r) => {
    M.tasks.add(t = { c: e, f: r });
  }), abort() {
    M.tasks.delete(t);
  } };
}
function Q(e, t) {
  Bt(() => {
    e.dispatchEvent(new CustomEvent(t));
  });
}
function Kt(e) {
  if (e === "float") return "cssFloat";
  if (e === "offset") return "cssOffset";
  if (e.startsWith("--")) return e;
  const t = e.split("-");
  return t.length === 1 ? t[0] : t[0] + t.slice(1).map((r) => r[0].toUpperCase() + r.slice(1)).join("");
}
function pe(e) {
  const t = {}, r = e.split(";");
  for (const a of r) {
    const [i, n] = a.split(":");
    if (!i || n === void 0) break;
    const s = Kt(i.trim());
    t[s] = n.trim();
  }
  return t;
}
const jt = (e) => e;
function We(e, t, r, a) {
  var i = (e & kt) !== 0, n = (e & Et) !== 0, s = i && n, o = (e & pt) !== 0, w = s ? "both" : i ? "in" : "out", l, c = t.inert, p = t.style.overflow, f, g;
  function u() {
    var h = mt, v = K;
    he(null), _e(null);
    try {
      return l ?? (l = r()(t, (a == null ? void 0 : a()) ?? {}, { direction: w }));
    } finally {
      he(h), _e(v);
    }
  }
  var y = { is_global: o, in() {
    var _a;
    if (t.inert = c, !i) {
      g == null ? void 0 : g.abort(), (_a = g == null ? void 0 : g.reset) == null ? void 0 : _a.call(g);
      return;
    }
    n || (f == null ? void 0 : f.abort()), Q(t, "introstart"), f = oe(t, u(), g, 1, () => {
      Q(t, "introend"), f == null ? void 0 : f.abort(), f = l = void 0, t.style.overflow = p;
    });
  }, out(h) {
    if (!n) {
      h == null ? void 0 : h(), l = void 0;
      return;
    }
    t.inert = true, Q(t, "outrostart"), g = oe(t, u(), f, 0, () => {
      Q(t, "outroend"), h == null ? void 0 : h();
    });
  }, stop: () => {
    f == null ? void 0 : f.abort(), g == null ? void 0 : g.abort();
  } }, E = K;
  if ((E.transitions ?? (E.transitions = [])).push(y), i && Ot) {
    var d = o;
    if (!d) {
      for (var A = E.parent; A && (A.f & Le) !== 0; ) for (; (A = A.parent) && (A.f & bt) === 0; ) ;
      d = !A || (A.f & wt) !== 0;
    }
    d && yt(() => {
      G(() => y.in());
    });
  }
}
function oe(e, t, r, a, i) {
  var n = a === 1;
  if (At(t)) {
    var s, o = false;
    return Re(() => {
      if (!o) {
        var E = t({ direction: n ? "in" : "out" });
        s = oe(e, E, r, a, i);
      }
    }), { abort: () => {
      o = true, s == null ? void 0 : s.abort();
    }, deactivate: () => s.deactivate(), reset: () => s.reset(), t: () => s.t() };
  }
  if (r == null ? void 0 : r.deactivate(), !(t == null ? void 0 : t.duration)) return i(), { abort: x, deactivate: x, reset: x, t: () => a };
  const { delay: w = 0, css: l, tick: c, easing: p = jt } = t;
  var f = [];
  if (n && r === void 0 && (c && c(0, 1), l)) {
    var g = pe(l(0, 1));
    f.push(g, g);
  }
  var u = () => 1 - a, y = e.animate(f, { duration: w });
  return y.onfinish = () => {
    var E = (r == null ? void 0 : r.t()) ?? 1 - a;
    r == null ? void 0 : r.abort();
    var d = a - E, A = t.duration * Math.abs(d), h = [];
    if (A > 0) {
      var v = false;
      if (l) for (var b = Math.ceil(A / 16.666666666666668), k = 0; k <= b; k += 1) {
        var _ = E + d * p(k / b), m = pe(l(_, 1 - _));
        h.push(m), v || (v = m.overflow === "hidden");
      }
      v && (e.style.overflow = "hidden"), u = () => {
        var C = y.currentTime;
        return E + d * p(C / A);
      }, c && Qt(() => {
        if (y.playState !== "running") return false;
        var C = u();
        return c(C, 1 - C), true;
      });
    }
    y = e.animate(h, { duration: A, fill: "forwards" }), y.onfinish = () => {
      u = () => a, c == null ? void 0 : c(a, 1 - a), i();
    };
  }, { abort: () => {
    y && (y.cancel(), y.effect = null, y.onfinish = x);
  }, deactivate: () => {
    i = x;
  }, reset: () => {
    a === 0 && (c == null ? void 0 : c(1, 0));
  }, t: () => u() };
}
function wr(e, t, r = t) {
  var a = Ct();
  Be(e, "input", (i) => {
    var n = i ? e.defaultValue : e.value;
    if (n = ie(e) ? ne(n) : n, r(n), a && n !== (n = t())) {
      var s = e.selectionStart, o = e.selectionEnd;
      e.value = n ?? "", o !== null && (e.selectionStart = s, e.selectionEnd = Math.min(o, e.value.length));
    }
  }), (R && e.defaultValue !== e.value || G(t) == null && e.value) && r(ie(e) ? ne(e.value) : e.value), Se(() => {
    var i = t();
    ie(e) && i === ne(e.value) || e.type === "date" && !i && !e.value || i !== e.value && (e.value = i ?? "");
  });
}
function yr(e, t, r = t) {
  Be(e, "change", (a) => {
    var i = a ? e.defaultChecked : e.checked;
    r(i);
  }), (R && e.defaultChecked !== e.checked || G(t) == null) && r(e.checked), Se(() => {
    var a = t();
    e.checked = !!a;
  });
}
function ie(e) {
  var t = e.type;
  return t === "number" || t === "range";
}
function ne(e) {
  return e === "" ? null : +e;
}
const Xt = (e) => e;
function Ge(e) {
  const t = e - 1;
  return t * t * t + 1;
}
function Qe(e, { delay: t = 0, duration: r = 400, easing: a = Xt } = {}) {
  const i = +getComputedStyle(e).opacity;
  return { delay: t, duration: r, easing: a, css: (n) => `opacity: ${n * i}` };
}
function pr(e, { delay: t = 0, duration: r = 400, easing: a = Ge, axis: i = "y" } = {}) {
  const n = getComputedStyle(e), s = +n.opacity, o = i === "y" ? "height" : "width", w = parseFloat(n[o]), l = i === "y" ? ["top", "bottom"] : ["left", "right"], c = l.map((d) => `${d[0].toUpperCase()}${d.slice(1)}`), p = parseFloat(n[`padding${c[0]}`]), f = parseFloat(n[`padding${c[1]}`]), g = parseFloat(n[`margin${c[0]}`]), u = parseFloat(n[`margin${c[1]}`]), y = parseFloat(n[`border${c[0]}Width`]), E = parseFloat(n[`border${c[1]}Width`]);
  return { delay: t, duration: r, easing: a, css: (d) => `overflow: hidden;opacity: ${Math.min(d * 20, 1) * s};${o}: ${d * w}px;padding-${l[0]}: ${d * p}px;padding-${l[1]}: ${d * f}px;margin-${l[0]}: ${d * g}px;margin-${l[1]}: ${d * u}px;border-${l[0]}-width: ${d * y}px;border-${l[1]}-width: ${d * E}px;min-${o}: 0` };
}
function ke(e, t) {
  for (const r in t) e[r] = t[r];
  return e;
}
function kr({ fallback: e, ...t }) {
  const r = /* @__PURE__ */ new Map(), a = /* @__PURE__ */ new Map();
  function i(s, o, w) {
    const { delay: l = 0, duration: c = (k) => Math.sqrt(k) * 30, easing: p = Ge } = ke(ke({}, t), w), f = s.getBoundingClientRect(), g = o.getBoundingClientRect(), u = f.left - g.left, y = f.top - g.top, E = f.width / g.width, d = f.height / g.height, A = Math.sqrt(u * u + y * y), h = getComputedStyle(o), v = h.transform === "none" ? "" : h.transform, b = +h.opacity;
    return { delay: l, duration: typeof c == "function" ? c(A) : c, easing: p, css: (k, _) => `
			   opacity: ${k * b};
			   transform-origin: top left;
			   transform: ${v} translate(${_ * u}px,${_ * y}px) scale(${k + (1 - k) * E}, ${k + (1 - k) * d});
		   ` };
  }
  function n(s, o, w) {
    return (l, c) => (s.set(c.key, l), () => {
      if (o.has(c.key)) {
        const p = o.get(c.key);
        return o.delete(c.key), i(p, l, c);
      }
      return s.delete(c.key), e && e(l, c, w);
    });
  }
  return [n(a, r, false), n(r, a, true)];
}
var Jt = U('<div><div class="loading svelte-1yqkxw6"><div class="loading-1 svelte-1yqkxw6"></div> <div class="loading-2 svelte-1yqkxw6"></div> <div class="loading-3 svelte-1yqkxw6"></div></div></div>');
function Zt(e, t) {
  let r = T(t, "background", 3, false), a = T(t, "color", 3, "var(--col-text)"), i = T(t, "global", 3, false), n = T(t, "offset", 3, 0);
  var s = Jt();
  let o;
  var w = N(s), l = N(w);
  let c;
  var p = j(l, 2);
  let f;
  var g = j(p, 2);
  let u;
  I(w), I(s), J((y) => {
    o = ze(s, 1, "container svelte-1yqkxw6", null, o, y), H(w, `margin-top: ${n() ?? ""}px;`), c = H(l, "", c, { background: a() }), f = H(p, "", f, { background: a() }), u = H(g, "", u, { background: a() });
  }, [() => ({ global: i(), local: !i(), background: r() })]), We(3, s, () => Qe, () => ({ duration: 100 })), F(e, s);
}
var $t = U('<div class="load svelte-1m0mzre"><!></div>'), er = U('<div class="font-label"><!></div>'), tr = U("<button><!></button>");
function Er(e, t) {
  Ne(t, true);
  let r = T(t, "type", 3, "button"), a = T(t, "role", 3, "button"), i = T(t, "ref", 15), n = T(t, "level", 3, 2), s = T(t, "isDisabled", 3, false), o = T(t, "isLoading", 3, false), w = T(t, "destructive", 3, false), l = T(t, "invisible", 3, false), c = T(t, "invisibleOutline", 3, false), p = Ft(t, ["$$slots", "$$events", "$$legacy", "type", "role", "ref", "id", "ariaLabel", "ariaControls", "ariaCurrent", "level", "width", "isDisabled", "isLoading", "destructive", "invisible", "invisibleOutline", "popovertarget", "popovertargetaction", "onclick", "onLeft", "onRight", "onUp", "onDown", "children"]), f = te(() => {
    if (l()) return "invisible";
    if (w()) return "destructive";
    switch (n()) {
      case 2:
        return "l2";
      case 3:
        return "l3";
      default:
        return "l1";
    }
  }), g = Y(!o()), u = te(() => s() || o());
  Ie(() => {
    o() ? setTimeout(() => {
      S(g, false);
    }, 120) : setTimeout(() => {
      S(g, true);
    }, 120);
  });
  function y() {
    if (w()) return "var(--btn-text)";
    switch (n()) {
      case 2:
        return "hsl(var(--action))";
      case 3:
        return "hsl(var(--action))";
      default:
        return "var(--btn-text)";
    }
  }
  function E(k) {
    var _a, _b, _c, _d, _e2;
    switch (k.code) {
      case "Enter":
        (_a = t.onclick) == null ? void 0 : _a.call(t, k);
        break;
      case "ArrowLeft":
        (_b = t.onLeft) == null ? void 0 : _b.call(t);
        break;
      case "ArrowRight":
        (_c = t.onRight) == null ? void 0 : _c.call(t);
        break;
      case "ArrowUp":
        (_d = t.onUp) == null ? void 0 : _d.call(t);
        break;
      case "ArrowDown":
        (_e2 = t.onDown) == null ? void 0 : _e2.call(t);
        break;
    }
  }
  var d = tr();
  let A;
  var h = N(d);
  {
    var v = (k) => {
      var _ = $t(), m = N(_);
      const C = te(y);
      Zt(m, { background: false, get color() {
        return L(C);
      } }), I(_), F(k, _);
    }, b = (k, _) => {
      {
        var m = (C) => {
          var O = er(), D = N(O);
          De(D, () => t.children), I(O), We(1, O, () => Qe), F(C, O);
        };
        X(k, (C) => {
          L(g) && C(m);
        }, _);
      }
    };
    X(h, (k) => {
      o() ? k(v) : k(b, false);
    });
  }
  I(d), qe(d, (k) => i(k), () => i()), J((k) => A = Wt(d, A, { role: a(), type: r(), id: t.id, "aria-label": t.ariaLabel, "aria-controls": t.ariaControls, "aria-current": t.ariaCurrent, class: L(f), "data-isloading": o(), onclick: t.onclick, onkeydown: E, disabled: L(u), "aria-disabled": L(u), popovertarget: t.popovertarget, popovertargetaction: t.popovertargetaction, ...p, [V]: k, [P]: { width: t.width } }, "svelte-1m0mzre"), [() => ({ invisibleOutline: c() })]), F(e, d), Me();
}
const rr = Tt(void 0), Ke = "/dashboard/api";
async function Ar(e) {
  let t = await fetch(`${Ke}${e}`, { method: "GET" });
  return je(t);
}
async function mr(e, t) {
  let r = await fetch(`${Ke}${e}`, { method: "POST", body: t });
  return je(r);
}
function je(e) {
  return e.status === 401 && rr.set(void 0), e;
}
var ar = qt(`<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.036 12.322a1.012 1.012 0 010-.639C3.423 7.51 7.36 4.5 12 4.5c4.638 0 8.573 3.007 9.963
            7.178.07.207.07.431 0 .639C20.577 16.49 16.64 19.5 12 19.5c-4.638 0-8.573-3.007-9.963-7.178z"></path><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"></path></svg>`);
function Cr(e, t) {
  let r = T(t, "color", 8, "var(--col-ok)"), a = T(t, "opacity", 8, 0.9), i = T(t, "width", 8, "1.5rem");
  var n = ar();
  q(n, "stroke-width", 2), J(() => {
    q(n, "width", i()), q(n, "color", r()), q(n, "opacity", a());
  }), F(e, n);
}
const ir = `-- comments will be ignored but only a single query is allowed
-- press CTRL + Enter to execute
SELECT 1`, nr = { id: "SELECT 1", query: ir }, Tr = "--!auto-query";
let Rr = se([nr]);
const Lr = (e) => {
  let t = "";
  const r = e || 8;
  for (let a = 0; a < r; a += 1) {
    let i = 60;
    for (; i > 57 && i < 65 || i > 90 && i < 97; ) i = Math.floor(Math.random() * 74) + 48;
    t = t.concat(String.fromCharCode(i));
  }
  return t;
};
function sr(e, t, r, a) {
  t(), window.addEventListener("mousemove", r), window.addEventListener("mouseup", a, { once: true });
}
function or(e, t, r, a) {
  t(), window.addEventListener("mousemove", r), window.addEventListener("mouseup", a, { once: true });
}
var lr = U('<div class="relative"><div role="none" class="right svelte-1u5iq19"></div></div>'), fr = U('<div class="relative"><div role="none" class="bottom svelte-1u5iq19"></div></div>'), ur = U('<div><div class="children svelte-1u5iq19"><div class="inner svelte-1u5iq19"><!></div> <!></div> <!></div>');
function Sr(e, t) {
  Ne(t, true);
  let r = T(t, "minWidthPx", 3, 50), a = T(t, "minHeightPx", 3, 50), i, n = Y(void 0), s = Y(void 0), o = Y(se(G(() => t.initialWidthPx))), w = Y(se(G(() => t.initialHeightPx)));
  Ie(() => {
    l();
  });
  function l() {
    var _a;
    if (i) {
      let _ = i.getBoundingClientRect();
      t.resizeRight && (S(s, _.left, true), S(o, _.width, true)), t.resizeBottom && (S(n, _.top, true), S(w, _.height, true)), (_a = t.onResizeBottom) == null ? void 0 : _a.call(t, _.bottom);
    }
  }
  function c() {
    window.removeEventListener("mousemove", p), l();
  }
  function p(_) {
    let m = window.scrollX + _.x - (L(s) || 0);
    m < r() ? S(o, r()) : S(o, m);
  }
  function f() {
    window.removeEventListener("mousemove", g), l();
  }
  function g(_) {
    console.log(window.screenY, _.y);
    let m = window.screenY + _.clientY - (L(n) || 0);
    if (m < a() ? S(w, a()) : S(w, m), i && t.onResizeBottom) {
      let C = i.getBoundingClientRect();
      t.onResizeBottom(C.bottom);
    }
  }
  var u = ur();
  let y;
  var E = N(u), d = N(E), A = N(d);
  De(A, () => t.children), I(d);
  var h = j(d, 2);
  {
    var v = (_) => {
      var m = lr(), C = N(m);
      C.__mousedown = [sr, l, p, c], I(m), F(_, m);
    };
    X(h, (_) => {
      t.resizeRight && _(v);
    });
  }
  I(E);
  var b = j(E, 2);
  {
    var k = (_) => {
      var m = fr(), C = N(m);
      C.__mousedown = [or, l, g, f], I(m), F(_, m);
    };
    X(b, (_) => {
      t.resizeBottom && _(k);
    });
  }
  I(u), qe(u, (_) => i = _, () => i), J(() => y = H(u, "", y, { width: L(o) && `${L(o)}px`, height: L(w) && `${L(w)}px`, border: t.border, padding: t.padding })), F(e, u), Me();
}
Oe(["mousedown"]);
export {
  Ke as A,
  Er as B,
  nr as D,
  Cr as I,
  Rr as Q,
  Sr as R,
  q as a,
  H as b,
  kr as c,
  wr as d,
  pr as e,
  rr as f,
  gr as g,
  je as h,
  _r as i,
  ze as j,
  Pt as k,
  Ar as l,
  Lr as m,
  Tr as n,
  Vt as o,
  yr as p,
  mr as q,
  br as r,
  De as s,
  We as t,
  ir as u
};
