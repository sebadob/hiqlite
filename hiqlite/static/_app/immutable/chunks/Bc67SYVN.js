import { q as ve, v as S, w as tt, g as L, X as rt, x as at, y as it, z as be, A as ae, B as W, G as z, ad as nt, ae as st, C as Le, D as ee, F as ot, af as ie, ag as Re, ah as X, ai as we, M as lt, aj as ye, ak as Me, al as de, am as ft, an as ut, ao as ct, ap as j, aq as _e, ar as vt, as as dt, at as _t, au as ht, av as Ne, K as Ie, aw as gt, I as te, ax as bt, ay as wt, P as yt, az as he, h as pt, aA as Et, aB as kt, aC as mt, aD as At, aE as Tt, aF as Ct, aG as Oe, aH as St, aI as Lt, aJ as Rt, aK as Mt, aL as Nt, aM as It, aN as Ot, E as qt, aO as Bt, aP as Ft, b as K, aQ as Dt, aR as Ut, aS as Pt, aT as xt, aU as zt, L as H, aV as pe, aW as Ee, aX as Ht, J as qe, aY as Yt, i as F, m as M, s as J, n as N, t as ge, k as B, p as Be, a8 as Y, a as Fe, l as De, R, ab as ne, aZ as Ue, a_ as Vt, a$ as Wt, Y as ue } from "./CYo-iuqb.js";
import "./NZTpNUN0.js";
import { p as C, i as Z, b as Pe, r as Gt } from "./mITizLRE.js";
import "./BXesWDf4.js";
function Sr(e, t) {
  return t;
}
function Kt(e, t, r, a) {
  for (var i = [], n = t.length, s = 0; s < n; s++) ft(t[s].e, i, true);
  var o = n > 0 && i.length === 0 && r !== null;
  if (o) {
    var y = r.parentNode;
    ut(y), y.append(r), a.clear(), O(e, t[0].prev, t[n - 1].next);
  }
  ct(i, () => {
    for (var f = 0; f < n; f++) {
      var c = t[f];
      o || (a.delete(c.k), O(e, c.prev, c.next)), j(c.e, !o);
    }
  });
}
function Lr(e, t, r, a, i, n = null) {
  var s = e, o = { flags: t, items: /* @__PURE__ */ new Map(), first: null }, y = (t & Ne) !== 0;
  if (y) {
    var f = e;
    s = S ? ae(_t(f)) : f.appendChild(ht());
  }
  S && tt();
  var c = null, p = false, l = rt(() => {
    var b = r();
    return Me(b) ? b : b == null ? [] : Re(b);
  });
  ve(() => {
    var b = L(l), u = b.length;
    if (p && u === 0) return;
    p = u === 0;
    let w = false;
    if (S) {
      var E = at(s) === it;
      E !== (u === 0) && (s = be(), ae(s), W(false), w = true);
    }
    if (S) {
      for (var _ = null, k, h = 0; h < u; h++) {
        if (z.nodeType === nt && z.data === st) {
          s = z, w = true, W(false);
          break;
        }
        var v = b[h], d = a(v, h);
        k = xe(z, o, _, null, v, d, h, i, t, r), o.items.set(d, k), _ = k;
      }
      u > 0 && ae(be());
    }
    S || Qt(b, o, s, i, t, a, r), n !== null && (u === 0 ? c ? Le(c) : c = ee(() => n(s)) : c !== null && ot(c, () => {
      c = null;
    })), w && W(true), L(l);
  }), S && (s = z);
}
function Qt(e, t, r, a, i, n, s) {
  var _a, _b, _c, _d;
  var o = (i & gt) !== 0, y = (i & (_e | de)) !== 0, f = e.length, c = t.items, p = t.first, l = p, b, u = null, w, E = [], _ = [], k, h, v, d;
  if (o) for (d = 0; d < f; d += 1) k = e[d], h = n(k, d), v = c.get(h), v !== void 0 && ((_a = v.a) == null ? void 0 : _a.measure(), (w ?? (w = /* @__PURE__ */ new Set())).add(v));
  for (d = 0; d < f; d += 1) {
    if (k = e[d], h = n(k, d), v = c.get(h), v === void 0) {
      var A = l ? l.e.nodes_start : r;
      u = xe(A, t, u, u === null ? t.first : u.next, k, h, d, a, i, s), c.set(h, u), E = [], _ = [], l = u.next;
      continue;
    }
    if (y && Xt(v, k, d, i), (v.e.f & ie) !== 0 && (Le(v.e), o && ((_b = v.a) == null ? void 0 : _b.unfix(), (w ?? (w = /* @__PURE__ */ new Set())).delete(v))), v !== l) {
      if (b !== void 0 && b.has(v)) {
        if (E.length < _.length) {
          var g = _[0], m;
          u = g.prev;
          var T = E[0], P = E[E.length - 1];
          for (m = 0; m < E.length; m += 1) ke(E[m], g, r);
          for (m = 0; m < _.length; m += 1) b.delete(_[m]);
          O(t, T.prev, P.next), O(t, u, T), O(t, P, g), l = g, u = P, d -= 1, E = [], _ = [];
        } else b.delete(v), ke(v, l, r), O(t, v.prev, v.next), O(t, v, u === null ? t.first : u.next), O(t, u, v), u = v;
        continue;
      }
      for (E = [], _ = []; l !== null && l.k !== h; ) (l.e.f & ie) === 0 && (b ?? (b = /* @__PURE__ */ new Set())).add(l), _.push(l), l = l.next;
      if (l === null) continue;
      v = l;
    }
    E.push(v), u = v, l = v.next;
  }
  if (l !== null || b !== void 0) {
    for (var x = b === void 0 ? [] : Re(b); l !== null; ) (l.e.f & ie) === 0 && x.push(l), l = l.next;
    var re = x.length;
    if (re > 0) {
      var et = (i & Ne) !== 0 && f === 0 ? r : null;
      if (o) {
        for (d = 0; d < re; d += 1) (_c = x[d].a) == null ? void 0 : _c.measure();
        for (d = 0; d < re; d += 1) (_d = x[d].a) == null ? void 0 : _d.fix();
      }
      Kt(t, x, et, c);
    }
  }
  o && Ie(() => {
    var _a2;
    if (w !== void 0) for (v of w) (_a2 = v.a) == null ? void 0 : _a2.apply();
  }), X.first = t.first && t.first.e, X.last = u && u.e;
}
function Xt(e, t, r, a) {
  (a & _e) !== 0 && we(e.v, t), (a & de) !== 0 ? we(e.i, r) : e.i = r;
}
function xe(e, t, r, a, i, n, s, o, y, f) {
  var c = (y & _e) !== 0, p = (y & vt) === 0, l = c ? p ? lt(i, false, false) : ye(i) : i, b = (y & de) === 0 ? s : ye(s), u = { i: b, v: l, k: n, a: null, e: null, prev: r, next: a };
  try {
    return u.e = ee(() => o(e, l, b, f), S), u.e.prev = r && r.e, u.e.next = a && a.e, r === null ? t.first = u : (r.next = u, r.e.next = u.e), a !== null && (a.prev = u, a.e.prev = u.e), u;
  } finally {
  }
}
function ke(e, t, r) {
  for (var a = e.next ? e.next.e.nodes_start : r, i = t ? t.e.nodes_start : r, n = e.e.nodes_start; n !== a; ) {
    var s = dt(n);
    i.before(n), n = s;
  }
}
function O(e, t, r) {
  t === null ? e.first = r : (t.next = r, t.e.next = r && r.e), r !== null && (r.prev = t, r.e.prev = t && t.e);
}
function jt(e, t) {
  var r = void 0, a;
  ve(() => {
    r !== (r = t()) && (a && (j(a), a = null), r && (a = ee(() => {
      te(() => r(e));
    })));
  });
}
function ze(e) {
  var t, r, a = "";
  if (typeof e == "string" || typeof e == "number") a += e;
  else if (typeof e == "object") if (Array.isArray(e)) {
    var i = e.length;
    for (t = 0; t < i; t++) e[t] && (r = ze(e[t])) && (a && (a += " "), a += r);
  } else for (r in e) e[r] && (a && (a += " "), a += r);
  return a;
}
function Jt() {
  for (var e, t, r = 0, a = "", i = arguments.length; r < i; r++) (e = arguments[r]) && (t = ze(e)) && (a && (a += " "), a += t);
  return a;
}
function Zt(e) {
  return typeof e == "object" ? Jt(e) : e ?? "";
}
const me = [...` 	
\r\f\xA0\v\uFEFF`];
function $t(e, t, r) {
  var a = e == null ? "" : "" + e;
  if (t && (a = a ? a + " " + t : t), r) {
    for (var i in r) if (r[i]) a = a ? a + " " + i : i;
    else if (a.length) for (var n = i.length, s = 0; (s = a.indexOf(i, s)) >= 0; ) {
      var o = s + n;
      (s === 0 || me.includes(a[s - 1])) && (o === a.length || me.includes(a[o])) ? a = (s === 0 ? "" : a.substring(0, s)) + a.substring(o + 1) : s = o;
    }
  }
  return a === "" ? null : a;
}
function Ae(e, t = false) {
  var r = t ? " !important;" : ";", a = "";
  for (var i in e) {
    var n = e[i];
    n != null && n !== "" && (a += " " + i + ": " + n + r);
  }
  return a;
}
function se(e) {
  return e[0] !== "-" || e[1] !== "-" ? e.toLowerCase() : e;
}
function er(e, t) {
  if (t) {
    var r = "", a, i;
    if (Array.isArray(t) ? (a = t[0], i = t[1]) : a = t, e) {
      e = String(e).replaceAll(/\s*\/\*.*?\*\/\s*/g, "").trim();
      var n = false, s = 0, o = false, y = [];
      a && y.push(...Object.keys(a).map(se)), i && y.push(...Object.keys(i).map(se));
      var f = 0, c = -1;
      const w = e.length;
      for (var p = 0; p < w; p++) {
        var l = e[p];
        if (o ? l === "/" && e[p - 1] === "*" && (o = false) : n ? n === l && (n = false) : l === "/" && e[p + 1] === "*" ? o = true : l === '"' || l === "'" ? n = l : l === "(" ? s++ : l === ")" && s--, !o && n === false && s === 0) {
          if (l === ":" && c === -1) c = p;
          else if (l === ";" || p === w - 1) {
            if (c !== -1) {
              var b = se(e.substring(f, c).trim());
              if (!y.includes(b)) {
                l !== ";" && p++;
                var u = e.substring(f, p).trim();
                r += " " + u + ";";
              }
            }
            f = p + 1, c = -1;
          }
        }
      }
    }
    return a && (r += Ae(a)), i && (r += Ae(i, true)), r = r.trim(), r === "" ? null : r;
  }
  return e == null ? null : String(e);
}
function He(e, t, r, a, i, n) {
  var s = e.__className;
  if (S || s !== r || s === void 0) {
    var o = $t(r, a, n);
    (!S || o !== e.getAttribute("class")) && (o == null ? e.removeAttribute("class") : t ? e.className = o : e.setAttribute("class", o)), e.__className = r;
  } else if (n && i !== n) for (var y in n) {
    var f = !!n[y];
    (i == null || f !== !!i[y]) && e.classList.toggle(y, f);
  }
  return n;
}
function oe(e, t = {}, r, a) {
  for (var i in r) {
    var n = r[i];
    t[i] !== n && (r[i] == null ? e.style.removeProperty(i) : e.style.setProperty(i, n, a));
  }
}
function U(e, t, r, a) {
  var i = e.__style;
  if (S || i !== t) {
    var n = er(t, a);
    (!S || n !== e.getAttribute("style")) && (n == null ? e.removeAttribute("style") : e.style.cssText = n), e.__style = t;
  } else a && (Array.isArray(a) ? (oe(e, r == null ? void 0 : r[0], a[0]), oe(e, r == null ? void 0 : r[1], a[1], "important")) : oe(e, r, a));
  return a;
}
function $(e, t, r) {
  if (e.multiple) {
    if (t == null) return;
    if (!Me(t)) return bt();
    for (var a of e.options) a.selected = t.includes(G(a));
    return;
  }
  for (a of e.options) {
    var i = G(a);
    if (wt(i, t)) {
      a.selected = true;
      return;
    }
  }
  (!r || t !== void 0) && (e.selectedIndex = -1);
}
function Ye(e) {
  var t = new MutationObserver(() => {
    $(e, e.__value);
  });
  t.observe(e, { childList: true, subtree: true, attributes: true, attributeFilter: ["value"] }), yt(() => {
    t.disconnect();
  });
}
function Rr(e, t, r = t) {
  var a = true;
  he(e, "change", (i) => {
    var n = i ? "[selected]" : ":checked", s;
    if (e.multiple) s = [].map.call(e.querySelectorAll(n), G);
    else {
      var o = e.querySelector(n) ?? e.querySelector("option:not([disabled])");
      s = o && G(o);
    }
    r(s);
  }), te(() => {
    var i = t();
    if ($(e, i, a), a && i === void 0) {
      var n = e.querySelector(":checked");
      n !== null && (i = G(n), r(i));
    }
    e.__value = i, a = false;
  }), Ye(e);
}
function G(e) {
  return "__value" in e ? e.__value : e.value;
}
const V = Symbol("class"), D = Symbol("style"), Ve = Symbol("is custom element"), We = Symbol("is html");
function Mr(e) {
  if (S) {
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
    e.__on_r = r, Nt(r), It();
  }
}
function tr(e, t) {
  t ? e.hasAttribute("selected") || e.setAttribute("selected", "") : e.removeAttribute("selected");
}
function q(e, t, r, a) {
  var i = Ge(e);
  S && (i[t] = e.getAttribute(t), t === "src" || t === "srcset" || t === "href" && e.nodeName === "LINK") || i[t] !== (i[t] = r) && (t === "loading" && (e[Rt] = r), r == null ? e.removeAttribute(t) : typeof r != "string" && Ke(e).includes(t) ? e[t] = r : e.setAttribute(t, r));
}
function rr(e, t, r, a, i = false) {
  var n = Ge(e), s = n[Ve], o = !n[We];
  let y = S && s;
  y && W(false);
  var f = t || {}, c = e.tagName === "OPTION";
  for (var p in t) p in r || (r[p] = null);
  r.class ? r.class = Zt(r.class) : r.class = null, r[D] && (r.style ?? (r.style = null));
  var l = Ke(e);
  for (const h in r) {
    let v = r[h];
    if (c && h === "value" && v == null) {
      e.value = e.__value = "", f[h] = v;
      continue;
    }
    if (h === "class") {
      var b = e.namespaceURI === "http://www.w3.org/1999/xhtml";
      He(e, b, v, a, t == null ? void 0 : t[V], r[V]), f[h] = v, f[V] = r[V];
      continue;
    }
    if (h === "style") {
      U(e, v, t == null ? void 0 : t[D], r[D]), f[h] = v, f[D] = r[D];
      continue;
    }
    var u = f[h];
    if (!(v === u && !(v === void 0 && e.hasAttribute(h)))) {
      f[h] = v;
      var w = h[0] + h[1];
      if (w !== "$$") if (w === "on") {
        const d = {}, A = "$$" + h;
        let g = h.slice(2);
        var E = Mt(g);
        if (Tt(g) && (g = g.slice(0, -7), d.capture = true), !E && u) {
          if (v != null) continue;
          e.removeEventListener(g, f[A], d), f[A] = null;
        }
        if (v != null) if (E) e[`__${g}`] = v, Oe([g]);
        else {
          let m = function(T) {
            f[h].call(this, T);
          };
          f[A] = Ct(g, e, m, d);
        }
        else E && (e[`__${g}`] = void 0);
      } else if (h === "style") q(e, h, v);
      else if (h === "autofocus") St(e, !!v);
      else if (!s && (h === "__value" || h === "value" && v != null)) e.value = e.__value = v;
      else if (h === "selected" && c) tr(e, v);
      else {
        var _ = h;
        o || (_ = Lt(_));
        var k = _ === "defaultValue" || _ === "defaultChecked";
        if (v == null && !s && !k) if (n[h] = null, _ === "value" || _ === "checked") {
          let d = e;
          const A = t === void 0;
          if (_ === "value") {
            let g = d.defaultValue;
            d.removeAttribute(_), d.defaultValue = g, d.value = d.__value = A ? g : null;
          } else {
            let g = d.defaultChecked;
            d.removeAttribute(_), d.defaultChecked = g, d.checked = A ? g : false;
          }
        } else e.removeAttribute(h);
        else k || l.includes(_) && (s || typeof v != "string") ? e[_] = v : typeof v != "function" && q(e, _, v);
      }
    }
  }
  return y && W(true), f;
}
function ar(e, t, r = [], a, i = false, n = pt) {
  const s = r.map(n);
  var o = void 0, y = {}, f = e.nodeName === "SELECT", c = false;
  if (ve(() => {
    var l = t(...s.map(L)), b = rr(e, o, l, a, i);
    c && f && "value" in l && $(e, l.value, false);
    for (let w of Object.getOwnPropertySymbols(y)) l[w] || j(y[w]);
    for (let w of Object.getOwnPropertySymbols(l)) {
      var u = l[w];
      w.description === mt && (!o || u !== o[w]) && (y[w] && j(y[w]), y[w] = ee(() => jt(e, () => u))), b[w] = u;
    }
    o = b;
  }), f) {
    var p = e;
    te(() => {
      $(p, o.value), Ye(p);
    });
  }
  c = true;
}
function Ge(e) {
  return e.__attributes ?? (e.__attributes = { [Ve]: e.nodeName.includes("-"), [We]: e.namespaceURI === Et });
}
var Te = /* @__PURE__ */ new Map();
function Ke(e) {
  var t = Te.get(e.nodeName);
  if (t) return t;
  Te.set(e.nodeName, t = []);
  for (var r, a = e, i = Element.prototype; i !== a; ) {
    r = At(a);
    for (var n in r) r[n].set && t.push(n);
    a = kt(a);
  }
  return t;
}
const ir = () => performance.now(), I = { tick: (e) => requestAnimationFrame(e), now: () => ir(), tasks: /* @__PURE__ */ new Set() };
function Qe() {
  const e = I.now();
  I.tasks.forEach((t) => {
    t.c(e) || (I.tasks.delete(t), t.f());
  }), I.tasks.size !== 0 && I.tick(Qe);
}
function nr(e) {
  let t;
  return I.tasks.size === 0 && I.tick(Qe), { promise: new Promise((r) => {
    I.tasks.add(t = { c: e, f: r });
  }), abort() {
    I.tasks.delete(t);
  } };
}
function Q(e, t) {
  xt(() => {
    e.dispatchEvent(new CustomEvent(t));
  });
}
function sr(e) {
  if (e === "float") return "cssFloat";
  if (e === "offset") return "cssOffset";
  if (e.startsWith("--")) return e;
  const t = e.split("-");
  return t.length === 1 ? t[0] : t[0] + t.slice(1).map((r) => r[0].toUpperCase() + r.slice(1)).join("");
}
function Ce(e) {
  const t = {}, r = e.split(";");
  for (const a of r) {
    const [i, n] = a.split(":");
    if (!i || n === void 0) break;
    const s = sr(i.trim());
    t[s] = n.trim();
  }
  return t;
}
const or = (e) => e;
function Xe(e, t, r, a) {
  var i = (e & Ut) !== 0, n = (e & Pt) !== 0, s = i && n, o = (e & Dt) !== 0, y = s ? "both" : i ? "in" : "out", f, c = t.inert, p = t.style.overflow, l, b;
  function u() {
    var h = Ht, v = X;
    pe(null), Ee(null);
    try {
      return f ?? (f = r()(t, (a == null ? void 0 : a()) ?? {}, { direction: y }));
    } finally {
      pe(h), Ee(v);
    }
  }
  var w = { is_global: o, in() {
    var _a;
    if (t.inert = c, !i) {
      b == null ? void 0 : b.abort(), (_a = b == null ? void 0 : b.reset) == null ? void 0 : _a.call(b);
      return;
    }
    n || (l == null ? void 0 : l.abort()), Q(t, "introstart"), l = ce(t, u(), b, 1, () => {
      Q(t, "introend"), l == null ? void 0 : l.abort(), l = f = void 0, t.style.overflow = p;
    });
  }, out(h) {
    if (!n) {
      h == null ? void 0 : h(), f = void 0;
      return;
    }
    t.inert = true, Q(t, "outrostart"), b = ce(t, u(), l, 0, () => {
      Q(t, "outroend"), h == null ? void 0 : h();
    });
  }, stop: () => {
    l == null ? void 0 : l.abort(), b == null ? void 0 : b.abort();
  } }, E = X;
  if ((E.transitions ?? (E.transitions = [])).push(w), i && Ot) {
    var _ = o;
    if (!_) {
      for (var k = E.parent; k && (k.f & qt) !== 0; ) for (; (k = k.parent) && (k.f & Bt) === 0; ) ;
      _ = !k || (k.f & Ft) !== 0;
    }
    _ && te(() => {
      K(() => w.in());
    });
  }
}
function ce(e, t, r, a, i) {
  var n = a === 1;
  if (zt(t)) {
    var s, o = false;
    return Ie(() => {
      if (!o) {
        var E = t({ direction: n ? "in" : "out" });
        s = ce(e, E, r, a, i);
      }
    }), { abort: () => {
      o = true, s == null ? void 0 : s.abort();
    }, deactivate: () => s.deactivate(), reset: () => s.reset(), t: () => s.t() };
  }
  if (r == null ? void 0 : r.deactivate(), !(t == null ? void 0 : t.duration)) return i(), { abort: H, deactivate: H, reset: H, t: () => a };
  const { delay: y = 0, css: f, tick: c, easing: p = or } = t;
  var l = [];
  if (n && r === void 0 && (c && c(0, 1), f)) {
    var b = Ce(f(0, 1));
    l.push(b, b);
  }
  var u = () => 1 - a, w = e.animate(l, { duration: y, fill: "forwards" });
  return w.onfinish = () => {
    w.cancel();
    var E = (r == null ? void 0 : r.t()) ?? 1 - a;
    r == null ? void 0 : r.abort();
    var _ = a - E, k = t.duration * Math.abs(_), h = [];
    if (k > 0) {
      var v = false;
      if (f) for (var d = Math.ceil(k / 16.666666666666668), A = 0; A <= d; A += 1) {
        var g = E + _ * p(A / d), m = Ce(f(g, 1 - g));
        h.push(m), v || (v = m.overflow === "hidden");
      }
      v && (e.style.overflow = "hidden"), u = () => {
        var T = w.currentTime;
        return E + _ * p(T / k);
      }, c && nr(() => {
        if (w.playState !== "running") return false;
        var T = u();
        return c(T, 1 - T), true;
      });
    }
    w = e.animate(h, { duration: k, fill: "forwards" }), w.onfinish = () => {
      u = () => a, c == null ? void 0 : c(a, 1 - a), i();
    };
  }, { abort: () => {
    w && (w.cancel(), w.effect = null, w.onfinish = H);
  }, deactivate: () => {
    i = H;
  }, reset: () => {
    a === 0 && (c == null ? void 0 : c(1, 0));
  }, t: () => u() };
}
function Nr(e, t, r = t) {
  var a = Yt();
  he(e, "input", (i) => {
    var n = i ? e.defaultValue : e.value;
    if (n = le(e) ? fe(n) : n, r(n), a && n !== (n = t())) {
      var s = e.selectionStart, o = e.selectionEnd;
      e.value = n ?? "", o !== null && (e.selectionStart = s, e.selectionEnd = Math.min(o, e.value.length));
    }
  }), (S && e.defaultValue !== e.value || K(t) == null && e.value) && r(le(e) ? fe(e.value) : e.value), qe(() => {
    var i = t();
    le(e) && i === fe(e.value) || e.type === "date" && !i && !e.value || i !== e.value && (e.value = i ?? "");
  });
}
function Ir(e, t, r = t) {
  he(e, "change", (a) => {
    var i = a ? e.defaultChecked : e.checked;
    r(i);
  }), (S && e.defaultChecked !== e.checked || K(t) == null) && r(e.checked), qe(() => {
    var a = t();
    e.checked = !!a;
  });
}
function le(e) {
  var t = e.type;
  return t === "number" || t === "range";
}
function fe(e) {
  return e === "" ? null : +e;
}
const lr = (e) => e;
function je(e) {
  const t = e - 1;
  return t * t * t + 1;
}
function Je(e, { delay: t = 0, duration: r = 400, easing: a = lr } = {}) {
  const i = +getComputedStyle(e).opacity;
  return { delay: t, duration: r, easing: a, css: (n) => `opacity: ${n * i}` };
}
function Or(e, { delay: t = 0, duration: r = 400, easing: a = je, axis: i = "y" } = {}) {
  const n = getComputedStyle(e), s = +n.opacity, o = i === "y" ? "height" : "width", y = parseFloat(n[o]), f = i === "y" ? ["top", "bottom"] : ["left", "right"], c = f.map((_) => `${_[0].toUpperCase()}${_.slice(1)}`), p = parseFloat(n[`padding${c[0]}`]), l = parseFloat(n[`padding${c[1]}`]), b = parseFloat(n[`margin${c[0]}`]), u = parseFloat(n[`margin${c[1]}`]), w = parseFloat(n[`border${c[0]}Width`]), E = parseFloat(n[`border${c[1]}Width`]);
  return { delay: t, duration: r, easing: a, css: (_) => `overflow: hidden;opacity: ${Math.min(_ * 20, 1) * s};${o}: ${_ * y}px;padding-${f[0]}: ${_ * p}px;padding-${f[1]}: ${_ * l}px;margin-${f[0]}: ${_ * b}px;margin-${f[1]}: ${_ * u}px;border-${f[0]}-width: ${_ * w}px;border-${f[1]}-width: ${_ * E}px;min-${o}: 0` };
}
function Se(e, t) {
  for (const r in t) e[r] = t[r];
  return e;
}
function qr({ fallback: e, ...t }) {
  const r = /* @__PURE__ */ new Map(), a = /* @__PURE__ */ new Map();
  function i(s, o, y) {
    const { delay: f = 0, duration: c = (A) => Math.sqrt(A) * 30, easing: p = je } = Se(Se({}, t), y), l = s.getBoundingClientRect(), b = o.getBoundingClientRect(), u = l.left - b.left, w = l.top - b.top, E = l.width / b.width, _ = l.height / b.height, k = Math.sqrt(u * u + w * w), h = getComputedStyle(o), v = h.transform === "none" ? "" : h.transform, d = +h.opacity;
    return { delay: f, duration: typeof c == "function" ? c(k) : c, easing: p, css: (A, g) => `
			   opacity: ${A * d};
			   transform-origin: top left;
			   transform: ${v} translate(${g * u}px,${g * w}px) scale(${A + (1 - A) * E}, ${A + (1 - A) * _});
		   ` };
  }
  function n(s, o, y) {
    return (f, c) => (s.set(c.key, f), () => {
      if (o.has(c.key)) {
        const p = o.get(c.key);
        return o.delete(c.key), i(p, f, c);
      }
      return s.delete(c.key), e && e(f, c, y);
    });
  }
  return [n(a, r, false), n(r, a, true)];
}
var fr = F('<div><div class="loading svelte-1yqkxw6"><div class="loading-1 svelte-1yqkxw6"></div> <div class="loading-2 svelte-1yqkxw6"></div> <div class="loading-3 svelte-1yqkxw6"></div></div></div>');
function ur(e, t) {
  let r = C(t, "background", 3, false), a = C(t, "color", 3, "var(--col-text)"), i = C(t, "global", 3, false), n = C(t, "offset", 3, 0);
  var s = fr();
  let o;
  var y = M(s), f = M(y);
  let c;
  var p = J(f, 2);
  let l;
  var b = J(p, 2);
  let u;
  N(y), N(s), ge((w, E, _, k) => {
    o = He(s, 1, "container svelte-1yqkxw6", null, o, w), U(y, `margin-top: ${n() ?? ""}px;`), c = U(f, "", c, E), l = U(p, "", l, _), u = U(b, "", u, k);
  }, [() => ({ global: i(), local: !i(), background: r() }), () => ({ background: a() }), () => ({ background: a() }), () => ({ background: a() })]), Xe(3, s, () => Je, () => ({ duration: 100 })), B(e, s);
}
var cr = F('<div class="load svelte-1m0mzre"><!></div>'), vr = F('<div class="font-label"><!></div>'), dr = F("<button><!></button>");
function Br(e, t) {
  Be(t, true);
  let r = C(t, "type", 3, "button"), a = C(t, "role", 3, "button"), i = C(t, "ref", 15), n = C(t, "level", 3, 2), s = C(t, "isDisabled", 3, false), o = C(t, "isLoading", 3, false), y = C(t, "destructive", 3, false), f = C(t, "invisible", 3, false), c = C(t, "invisibleOutline", 3, false), p = Gt(t, ["$$slots", "$$events", "$$legacy", "type", "role", "ref", "id", "ariaLabel", "ariaControls", "ariaCurrent", "level", "width", "isDisabled", "isLoading", "destructive", "invisible", "invisibleOutline", "popovertarget", "popovertargetaction", "onclick", "onLeft", "onRight", "onUp", "onDown", "children"]), l = ne(() => {
    if (f()) return "invisible";
    if (y()) return "destructive";
    switch (n()) {
      case 2:
        return "l2";
      case 3:
        return "l3";
      default:
        return "l1";
    }
  }), b = Y(!o()), u = ne(() => s() || o());
  Fe(() => {
    o() ? setTimeout(() => {
      R(b, false);
    }, 120) : setTimeout(() => {
      R(b, true);
    }, 120);
  });
  function w() {
    if (y()) return "var(--btn-text)";
    switch (n()) {
      case 2:
        return "hsl(var(--action))";
      case 3:
        return "hsl(var(--action))";
      default:
        return "var(--btn-text)";
    }
  }
  function E(d) {
    var _a, _b, _c, _d, _e2;
    switch (d.code) {
      case "Enter":
        (_a = t.onclick) == null ? void 0 : _a.call(t, d);
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
  var _ = dr();
  ar(_, (d, A) => ({ role: a(), type: r(), id: t.id, "aria-label": t.ariaLabel, "aria-controls": t.ariaControls, "aria-current": t.ariaCurrent, class: L(l), "data-isloading": o(), onclick: t.onclick, onkeydown: E, disabled: L(u), "aria-disabled": L(u), popovertarget: t.popovertarget, popovertargetaction: t.popovertargetaction, ...p, [V]: d, [D]: A }), [() => ({ invisibleOutline: c() }), () => ({ width: t.width })], "svelte-1m0mzre");
  var k = M(_);
  {
    var h = (d) => {
      var A = cr(), g = M(A);
      const m = ne(w);
      ur(g, { background: false, get color() {
        return L(m);
      } }), N(A), B(d, A);
    }, v = (d, A) => {
      {
        var g = (m) => {
          var T = vr(), P = M(T);
          Ue(P, () => t.children), N(T), Xe(1, T, () => Je), B(m, T);
        };
        Z(d, (m) => {
          L(b) && m(g);
        }, A);
      }
    };
    Z(k, (d) => {
      o() ? d(h) : d(v, false);
    });
  }
  N(_), Pe(_, (d) => i(d), () => i()), B(e, _), De();
}
const _r = Vt(void 0), Ze = "/dashboard/api";
async function Fr(e) {
  let t = await fetch(`${Ze}${e}`, { method: "GET" });
  return $e(t);
}
async function Dr(e, t) {
  let r = await fetch(`${Ze}${e}`, { method: "POST", body: t });
  return $e(r);
}
function $e(e) {
  return e.status === 401 && _r.set(void 0), e;
}
var hr = Wt(`<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.036 12.322a1.012 1.012 0 010-.639C3.423 7.51 7.36 4.5 12 4.5c4.638 0 8.573 3.007 9.963
            7.178.07.207.07.431 0 .639C20.577 16.49 16.64 19.5 12 19.5c-4.638 0-8.573-3.007-9.963-7.178z"></path><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"></path></svg>`);
function Ur(e, t) {
  let r = C(t, "color", 8, "var(--col-ok)"), a = C(t, "opacity", 8, 0.9), i = C(t, "width", 8, "1.5rem");
  var n = hr();
  q(n, "stroke-width", 2), ge(() => {
    q(n, "width", i()), q(n, "color", r()), q(n, "opacity", a());
  }), B(e, n);
}
const gr = `-- comments will be ignored but only a single query is allowed
-- press CTRL + Enter to execute
SELECT 1`, br = { id: "SELECT 1", query: gr }, Pr = "--!auto-query";
let xr = ue([br]);
const zr = (e) => {
  let t = "";
  const r = e || 8;
  for (let a = 0; a < r; a += 1) {
    let i = 60;
    for (; i > 57 && i < 65 || i > 90 && i < 97; ) i = Math.floor(Math.random() * 74) + 48;
    t = t.concat(String.fromCharCode(i));
  }
  return t;
};
function wr(e, t, r, a) {
  t(), window.addEventListener("mousemove", r), window.addEventListener("mouseup", a, { once: true });
}
function yr(e, t, r, a) {
  t(), window.addEventListener("mousemove", r), window.addEventListener("mouseup", a, { once: true });
}
var pr = F('<div class="relative"><div role="none" class="right svelte-1u5iq19"></div></div>'), Er = F('<div class="relative"><div role="none" class="bottom svelte-1u5iq19"></div></div>'), kr = F('<div><div class="children svelte-1u5iq19"><div class="inner svelte-1u5iq19"><!></div> <!></div> <!></div>');
function Hr(e, t) {
  Be(t, true);
  let r = C(t, "minWidthPx", 3, 50), a = C(t, "minHeightPx", 3, 50), i, n = Y(void 0), s = Y(void 0), o = Y(ue(K(() => t.initialWidthPx))), y = Y(ue(K(() => t.initialHeightPx)));
  Fe(() => {
    f();
  });
  function f() {
    var _a;
    if (i) {
      let g = i.getBoundingClientRect();
      t.resizeRight && (R(s, g.left, true), R(o, g.width, true)), t.resizeBottom && (R(n, g.top, true), R(y, g.height, true)), (_a = t.onResizeBottom) == null ? void 0 : _a.call(t, g.bottom);
    }
  }
  function c() {
    window.removeEventListener("mousemove", p), f();
  }
  function p(g) {
    let m = window.scrollX + g.x - (L(s) || 0);
    m < r() ? R(o, r()) : R(o, m);
  }
  function l() {
    window.removeEventListener("mousemove", b), f();
  }
  function b(g) {
    console.log(window.screenY, g.y);
    let m = window.screenY + g.clientY - (L(n) || 0);
    if (m < a() ? R(y, a()) : R(y, m), i && t.onResizeBottom) {
      let T = i.getBoundingClientRect();
      t.onResizeBottom(T.bottom);
    }
  }
  var u = kr();
  let w;
  var E = M(u), _ = M(E), k = M(_);
  Ue(k, () => t.children), N(_);
  var h = J(_, 2);
  {
    var v = (g) => {
      var m = pr(), T = M(m);
      T.__mousedown = [wr, f, p, c], N(m), B(g, m);
    };
    Z(h, (g) => {
      t.resizeRight && g(v);
    });
  }
  N(E);
  var d = J(E, 2);
  {
    var A = (g) => {
      var m = Er(), T = M(m);
      T.__mousedown = [yr, f, b, l], N(m), B(g, m);
    };
    Z(d, (g) => {
      t.resizeBottom && g(A);
    });
  }
  N(u), Pe(u, (g) => i = g, () => i), ge((g) => w = U(u, "", w, g), [() => ({ width: L(o) && `${L(o)}px`, height: L(y) && `${L(y)}px`, border: t.border, padding: t.padding })]), B(e, u), De();
}
Oe(["mousedown"]);
export {
  Ze as A,
  Br as B,
  br as D,
  Ur as I,
  xr as Q,
  Hr as R,
  U as a,
  Nr as b,
  qr as c,
  Or as d,
  _r as e,
  Lr as f,
  He as g,
  $e as h,
  Sr as i,
  Zt as j,
  Fr as k,
  zr as l,
  Pr as m,
  tr as n,
  Rr as o,
  Ir as p,
  Dr as q,
  Mr as r,
  q as s,
  Xe as t,
  gr as u
};
