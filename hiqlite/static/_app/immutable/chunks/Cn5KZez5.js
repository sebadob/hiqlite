var __defProp = Object.defineProperty;
var __typeError = (msg) => {
  throw TypeError(msg);
};
var __defNormalProp = (obj, key, value) => key in obj ? __defProp(obj, key, { enumerable: true, configurable: true, writable: true, value }) : obj[key] = value;
var __publicField = (obj, key, value) => __defNormalProp(obj, typeof key !== "symbol" ? key + "" : key, value);
var __accessCheck = (obj, member, msg) => member.has(obj) || __typeError("Cannot " + msg);
var __privateGet = (obj, member, getter) => (__accessCheck(obj, member, "read from private field"), getter ? getter.call(obj) : member.get(obj));
var __privateAdd = (obj, member, value) => member.has(obj) ? __typeError("Cannot add the same private member more than once") : member instanceof WeakSet ? member.add(obj) : member.set(obj, value);
var __privateSet = (obj, member, value, setter) => (__accessCheck(obj, member, "write to private field"), setter ? setter.call(obj, value) : member.set(obj, value), value);
var _s, _r, _e2, _t, _a, _i, _n;
import { m as A, n as U, o as g, q as Y, v as R, w as T, x as P, y as F, z as C, A as $, B as k, C as q, E as K, D as z, H as G, F as H, G as Z, I, J, K as Q, b as D, L as V, S as x, M as w, N as W, O as X, P as j, g as p, Q as ee, R as se, T as M, U as re, V as te, W as ae, h as ie, X as ne, Y as fe, Z as ue, _ as ce, $ as de, a0 as oe, a1 as le, a2 as he, a3 as _e, a4 as ve, a5 as be } from "./TbIIo73h.js";
class pe {
  constructor(s, t = true) {
    __publicField(this, "anchor");
    __privateAdd(this, _s, /* @__PURE__ */ new Map());
    __privateAdd(this, _r, /* @__PURE__ */ new Map());
    __privateAdd(this, _e2, /* @__PURE__ */ new Map());
    __privateAdd(this, _t, /* @__PURE__ */ new Set());
    __privateAdd(this, _a, true);
    __privateAdd(this, _i, () => {
      var s = A;
      if (__privateGet(this, _s).has(s)) {
        var t = __privateGet(this, _s).get(s), r = __privateGet(this, _r).get(t);
        if (r) U(r), __privateGet(this, _t).delete(t);
        else {
          var i = __privateGet(this, _e2).get(t);
          i && (__privateGet(this, _r).set(t, i.effect), __privateGet(this, _e2).delete(t), i.fragment.lastChild.remove(), this.anchor.before(i.fragment), r = i.effect);
        }
        for (const [a, f] of __privateGet(this, _s)) {
          if (__privateGet(this, _s).delete(a), a === s) break;
          const n = __privateGet(this, _e2).get(f);
          n && (g(n.effect), __privateGet(this, _e2).delete(f));
        }
        for (const [a, f] of __privateGet(this, _r)) {
          if (a === t || __privateGet(this, _t).has(a)) continue;
          const n = () => {
            if (Array.from(__privateGet(this, _s).values()).includes(a)) {
              var d = document.createDocumentFragment();
              C(f, d), d.append(R()), __privateGet(this, _e2).set(a, { effect: f, fragment: d });
            } else g(f);
            __privateGet(this, _t).delete(a), __privateGet(this, _r).delete(a);
          };
          __privateGet(this, _a) || !r ? (__privateGet(this, _t).add(a), Y(f, n, false)) : n();
        }
      }
    });
    __privateAdd(this, _n, (s) => {
      __privateGet(this, _s).delete(s);
      const t = Array.from(__privateGet(this, _s).values());
      for (const [r, i] of __privateGet(this, _e2)) t.includes(r) || (g(i.effect), __privateGet(this, _e2).delete(r));
    });
    this.anchor = s, __privateSet(this, _a, t);
  }
  ensure(s, t) {
    var r = A, i = $();
    if (t && !__privateGet(this, _r).has(s) && !__privateGet(this, _e2).has(s)) if (i) {
      var a = document.createDocumentFragment(), f = R();
      a.append(f), __privateGet(this, _e2).set(s, { effect: T(() => t(f)), fragment: a });
    } else __privateGet(this, _r).set(s, T(() => t(this.anchor)));
    if (__privateGet(this, _s).set(r, s), i) {
      for (const [n, u] of __privateGet(this, _r)) n === s ? r.skipped_effects.delete(u) : r.skipped_effects.add(u);
      for (const [n, u] of __privateGet(this, _e2)) n === s ? r.skipped_effects.delete(u.effect) : r.skipped_effects.add(u.effect);
      r.oncommit(__privateGet(this, _i)), r.ondiscard(__privateGet(this, _n));
    } else P && (this.anchor = F), __privateGet(this, _i).call(this);
  }
}
_s = new WeakMap();
_r = new WeakMap();
_e2 = new WeakMap();
_t = new WeakMap();
_a = new WeakMap();
_i = new WeakMap();
_n = new WeakMap();
function me(e, s, t = false) {
  P && q();
  var r = new pe(e), i = t ? K : 0;
  function a(f, n) {
    if (P) {
      const d = z(e) === G;
      if (f === d) {
        var u = H();
        Z(u), r.anchor = u, I(false), r.ensure(f, n), I(true);
        return;
      }
    }
    r.ensure(f, n);
  }
  k(() => {
    var f = false;
    s((n, u = true) => {
      f = true, a(u, n);
    }), f || a(false, null);
  }, i);
}
function O(e, s) {
  return e === s || (e == null ? void 0 : e[x]) === s;
}
function Ee(e = {}, s, t, r) {
  return J(() => {
    var i, a;
    return Q(() => {
      i = a, a = (r == null ? void 0 : r()) || [], D(() => {
        e !== t(...a) && (s(e, ...a), i && O(t(...i), e) && s(null, ...i));
      });
    }), () => {
      V(() => {
        a && O(t(...a), e) && s(null, ...a);
      });
    };
  }), e;
}
let b = false, m = Symbol();
function ye(e, s, t) {
  const r = t[s] ?? (t[s] = { store: null, source: W(void 0), unsubscribe: w });
  if (r.store !== e && !(m in t)) if (r.unsubscribe(), r.store = e ?? null, e == null) r.source.v = void 0, r.unsubscribe = w;
  else {
    var i = true;
    r.unsubscribe = X(e, (a) => {
      i ? r.source.v = a : M(r.source, a);
    }), i = false;
  }
  return e && m in t ? j(e) : p(r.source);
}
function Ae() {
  const e = {};
  function s() {
    ee(() => {
      for (var t in e) e[t].unsubscribe();
      se(e, m, { enumerable: false, value: true });
    });
  }
  return [e, s];
}
function Se(e) {
  var s = b;
  try {
    return b = false, [e(), b];
  } finally {
    b = s;
  }
}
const ge = { get(e, s) {
  if (!e.exclude.includes(s)) return e.props[s];
}, set(e, s) {
  return false;
}, getOwnPropertyDescriptor(e, s) {
  if (!e.exclude.includes(s) && s in e.props) return { enumerable: true, configurable: true, value: e.props[s] };
}, has(e, s) {
  return e.exclude.includes(s) ? false : s in e.props;
}, ownKeys(e) {
  return Reflect.ownKeys(e.props).filter((s) => !e.exclude.includes(s));
} };
function Re(e, s, t) {
  return new Proxy({ props: e, exclude: s }, ge);
}
function Te(e, s, t, r) {
  var _a2;
  var i = !le || (t & he) !== 0, a = (t & oe) !== 0, f = (t & ve) !== 0, n = r, u = true, d = () => (u && (u = false, n = f ? D(r) : r), n), l;
  if (a) {
    var N = x in e || be in e;
    l = ((_a2 = re(e, s)) == null ? void 0 : _a2.set) ?? (N && s in e ? (c) => e[s] = c : void 0);
  }
  var h, E = false;
  a ? [h, E] = Se(() => e[s]) : h = e[s], h === void 0 && r !== void 0 && (h = d(), l && (i && te(), l(h)));
  var o;
  if (i ? o = () => {
    var c = e[s];
    return c === void 0 ? d() : (u = true, c);
  } : o = () => {
    var c = e[s];
    return c !== void 0 && (n = void 0), c === void 0 ? n : c;
  }, i && (t & ae) === 0) return o;
  if (l) {
    var L = e.$$legacy;
    return (function(c, v) {
      return arguments.length > 0 ? ((!i || !v || L || E) && l(v ? o() : c), c) : o();
    });
  }
  var S = false, _ = ((t & _e) !== 0 ? ie : ne)(() => (S = false, o()));
  a && p(_);
  var B = ce;
  return (function(c, v) {
    if (arguments.length > 0) {
      const y = v ? p(_) : i && a ? fe(c) : c;
      return M(_, y), S = true, n !== void 0 && (n = y), c;
    }
    return ue && S || (B.f & de) !== 0 ? _.v : p(_);
  });
}
export {
  pe as B,
  ye as a,
  Ee as b,
  me as i,
  Te as p,
  Re as r,
  Ae as s
};
